/*
Implement the OpenAI API generically for any LLM following it
Chris Kennedy @2024 MIT license
*/

#[cfg(feature = "ndi")]
use crate::ndi::send_images_over_ndi;
use crate::stable_diffusion::{sd, SDConfig};
use bytes::Bytes;
use chrono::NaiveDateTime;
use log::{debug, error, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::time::Instant;
use tokio::sync::mpsc::{self};

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct OpenAIRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<Message>,
    pub max_tokens: &'a i32,        // add this field to the request struct
    pub temperature: &'a f32,       // add this field to the request struct
    pub top_p: &'a f32,             // add this field to the request struct
    pub presence_penalty: &'a f32,  // add this field to the request struct
    pub frequency_penalty: &'a f32, // add this field to the request struct
    pub stream: &'a bool,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    role: Option<String>,
    created: Option<i64>,
    id: Option<String>,
    model: Option<String>,
    object: Option<String>,
    choices: Option<Vec<Choice>>,
    content: Option<String>,
    system_fingerprint: Option<String>,
}

#[derive(Deserialize)]
pub struct Choice {
    finish_reason: Option<String>,
    logprobs: Option<bool>,
    index: i32,
    delta: Delta, // Use Option to handle cases where it might be null or missing
}

#[derive(Debug, Deserialize)]
pub struct Delta {
    content: Option<String>,
}

// Function to process and print content tokens.
fn process_and_print_token(token: &str, current_line_length: &mut usize, break_line_length: usize) {
    let token_length = token.chars().count();

    // Check if adding this token exceeds the line length limit.
    if *current_line_length + token_length > break_line_length {
        // Find an opportune break point in the token if it's too long by itself.
        if token_length > break_line_length {
            let mut last_break_point = 0;
            for (i, c) in token.char_indices() {
                if c.is_ascii_punctuation() || c.is_whitespace() {
                    last_break_point = i;
                }
                // Break the token at the last break point if exceeding the line length.
                if i - last_break_point > break_line_length {
                    println!("{}", &token[last_break_point..i]);
                    *current_line_length = 0; // Reset the line length after printing.
                    last_break_point = i; // Update the last break point.
                }
            }
            // Print the remaining part of the token.
            print!("{}", &token[last_break_point..]);
            *current_line_length = token_length - last_break_point;
        } else {
            // If the current token doesn't exceed the limit by itself, just break the line before printing it.
            println!(); // Print a newline to break the line.
            print!("{}", token); // Print the current token at the start of a new line.
            *current_line_length = token_length; // Reset the line length to the current token's length.
        }
    } else {
        // If adding the token doesn't exceed the limit, just print it.
        print!("{}", token);
        *current_line_length += token_length; // Update the current line length.
    }
}

/*
 * {"choices":[{"finish_reason":"stop","index":0,"message":{"content":"The Los Angeles Dodgers won
 * the World Series in 2020. They defeated the Tampa Bay Rays in six
 * games.","role":"assistant"}}],"created":1706900958,"id":"chatcmpl-8jqjxqYj1IkKixqlHVvmTyJynoPOjaoA","model":"gpt-3.5-turbo","object":"chat.completion","usage":{"completion_tokens":30,"prompt_tokens":62,"total_tokens":92}}
 */

pub async fn stream_completion(
    open_ai_request: OpenAIRequest<'_>,
    openai_key: &str,
    llm_host: &str,
    llm_path: &str,
    debug_inline: bool,
    show_output_errors: bool,
    break_line_length: usize,
    sd_image: bool,
    ndi_images: bool,
) -> Result<Vec<Message>, Box<dyn std::error::Error>> {
    let client = Client::new();

    // measure messages member size of the content member of each pair of the messages array
    let mut prompt_token_count = 0;
    for message in &open_ai_request.messages {
        prompt_token_count += message.content.split_whitespace().count();
    }

    let mut response_messages = Vec::new(); // Collect messages here

    let start_time = Instant::now();
    let mut response = client
        .post(format!("{}{}", llm_host, llm_path))
        .header("Authorization", format!("Bearer {}", openai_key))
        .json(&open_ai_request)
        .send()
        .await?;

    // handle errors
    match response.error_for_status_ref() {
        Ok(_) => (),
        Err(e) => {
            println!("Error: {}", e);
            return Err(Box::new(e));
        }
    }

    let mut token_count = 0;
    let mut byte_count = 0;
    let mut current_line_length = 0;
    let mut loop_count = 0;
    // errors are strings

    if !open_ai_request.stream {
        info!("Response status: {}", response.status());
        debug!("Headers: {:#?}", response.headers());
        println!("\nLLM Response:\n  {}\n---\n", response.text().await?);
    } else {
        // Create an mpsc channel
        let (tx, mut rx) = mpsc::channel::<Bytes>(32);
        let (etx, mut erx) = mpsc::channel::<String>(32);

        let headers = response.headers().clone(); // Clone the headers
        let status = response.status(); // Copy the status as well since it's Copy

        // loop through the chunks
        // Spawn a new task for each chunk to process it asynchronously
        let worker = tokio::spawn(async move {
            let mut first_run = true;
            while let Some(chunk) = rx.recv().await {
                loop_count += 1;

                if first_run {
                    // print headers properly without causing a borrow error
                    debug!("Headers: {:#?}", headers);
                    info!("Response status: {}", status);
                }

                first_run = false;

                debug!("#{} LLM Result Chunk: {:#?}\n", loop_count, chunk);
                let chunk_vec = Vec::from(chunk.as_ref());
                let chunk_str = match String::from_utf8(chunk_vec).ok() {
                    Some(s) => s,
                    None => {
                        error!(
                            "Invalid UTF-8 sequence, skipping chunk. {}/{:?}",
                            chunk.len(),
                            chunk
                        );
                        continue;
                    } // skip non-UTF-8 chunks
                };

                // Splitting the chunk based on "data: " prefix to handle multiple JSON blobs
                let json_blobs: Vec<&str> = chunk_str.split("\ndata: ").collect();
                let mut blob_count = 0;

                for json_blob in json_blobs.iter() {
                    blob_count += 1;
                    debug!("Json Blob: {}/{} - {}", loop_count, blob_count, json_blob);
                    if json_blob.is_empty() || *json_blob == "\n" {
                        debug!("Empty line in response chunks.");
                        continue;
                    }

                    if json_blob == &"[DONE]" {
                        info!("End of response chunks.\n");
                        break;
                    }

                    // Confirm we have a '{' at the start, or find the offset of first '{' character
                    let offset = json_blob.find('{').unwrap_or(0);
                    let response_json = &json_blob[offset..];

                    if response_json.is_empty() {
                        error!("Invalid response chunk:\n - '{}'", json_blob);
                        continue;
                    }

                    debug!("Chunk #{} response: '{}'", loop_count, response_json);

                    match serde_json::from_str::<OpenAIResponse>(response_json) {
                        Ok(res) => {
                            let content = match &res.content {
                                Some(content) => content,
                                None => "",
                            };

                            if !content.is_empty() {
                                println!("LLM Content Response: {}", content);
                            }

                            // if res.content exists then continue to the next chunk
                            if res.content.is_some() {
                                continue;
                            }

                            // Assume `res` is an instance of `OpenAIResponse` you've deserialized
                            let choices = &res.choices.unwrap_or_else(|| {
                                error!("No choices found in response.");
                                Vec::new() // Provide a default value that matches the expected type
                            });

                            let role = match res.role {
                                Some(role) => role,
                                None => "unknown".to_string(),
                            };

                            if let Some(choice) = choices.get(0) {
                                // check if we got the created date from res.created, if so convert it to naivedatatime for usage else use a default value
                                let created_date = match res.created {
                                    Some(created_timestamp) => {
                                        NaiveDateTime::from_timestamp_opt(created_timestamp, 0)
                                            .map(|dt| dt.to_string())
                                            .unwrap_or_else(|| "unknown".to_string())
                                    }
                                    None => "unknown".to_string(),
                                };

                                let id = match res.id {
                                    Some(id) => id,
                                    None => "unknown".to_string(),
                                };

                                let model = match res.model {
                                    Some(model) => model,
                                    None => "unknown".to_string(),
                                };

                                let object = match res.object {
                                    Some(object) => object,
                                    None => "unknown".to_string(),
                                };

                                // check if we have a finish reason
                                if let Some(reason) = &choice.finish_reason {
                                    let end_time = Instant::now();
                                    let mut duration = end_time.duration_since(start_time);
                                    let pretty_time = format!("{:?}", duration);

                                    // Ensure the duration is at least 1 second
                                    if duration < std::time::Duration::new(1, 0) {
                                        duration = std::time::Duration::new(1, 0);
                                    }

                                    info!(
                                        "\n--\nIndex {} ID {}\nObject {} by Model {} User {}\nCreated on {} Finish reason: {}\n {}/{}/{} Tokens/Prompt/Response {} Bytes at {} tokens per second and {} seconds to complete.\n--\n",
                                        choice.index,
                                        id,
                                        object,
                                        model,
                                        role,
                                        created_date,
                                        reason,
                                        token_count + prompt_token_count,
                                        prompt_token_count,
                                        token_count,
                                        byte_count,
                                        token_count as u64 / duration.as_secs(),
                                        pretty_time
                                    );

                                    // break the loop if we have a finish reason
                                    break;
                                }

                                // check for system_fingerprint
                                if let Some(fingerprint) = &res.system_fingerprint {
                                    debug!("\nSystem fingerprint: {}", fingerprint);
                                }

                                // check for logprobs
                                if let Some(logprobs) = choice.logprobs {
                                    println!("Logprobs: {}", logprobs);
                                }

                                // check if we have content in the delta
                                if let Some(content) = &choice.delta.content {
                                    token_count += 1;
                                    byte_count += content.len();
                                    etx.send(format!("{}", content))
                                        .await
                                        .expect("Failed to send content");

                                    process_and_print_token(
                                        content,
                                        &mut current_line_length,
                                        break_line_length,
                                    );

                                    // flush stdout
                                    std::io::stdout().flush().unwrap();
                                }
                            } else {
                                error!("No choices available.");
                            }
                        }
                        Err(e) => {
                            // Handle the parse error here
                            if debug_inline {
                                error!("\nFailed to parse response: {}\n", e);
                                error!("\nResponse that failed to parse: '{}'\n", response_json);
                            } else {
                                // push to etx channel
                                if show_output_errors {
                                    etx.send(format!("ERROR: {} - {}", e, response_json))
                                        .await
                                        .expect("Failed to send error");
                                    print!(".X.");
                                }
                            }
                        }
                    }
                }
            }
        });

        // collect answers from the worker
        let error_collector = tokio::spawn(async move {
            let mut errors = Vec::new();
            let mut answers = Vec::new();
            while let Some(message) = erx.recv().await {
                if message.starts_with("ERROR:") {
                    errors.push(message);
                } else {
                    // check if there is a period at the end of the message
                    if message.ends_with('.') {
                        // send to stable diffusion as a separate thread to avoid blocking
                    }
                    answers.push(message);
                }
            }
            (errors, answers) // Return collected errors and answers from the task
        });

        // Main task to send chunks to the worker
        while let Some(chunk) = response.chunk().await? {
            tx.send(chunk).await.expect("Failed to send chunk");
        }

        // Close the channel by dropping tx
        drop(tx);

        // Await the worker task to finish processing
        worker.await?;

        // Await the error collector task to retrieve the collected errors and answers
        let (errors, answers) = error_collector
            .await
            .unwrap_or_else(|_| (Vec::new(), Vec::new())); // Handle errors by returning empty vectors

        // Print errors
        if !errors.is_empty() {
            println!("\nErrors:");
            for error in errors.iter() {
                println!("{}", error);
            }
        }

        // Store LLM complete answer from the worker task
        response_messages.push(Message {
            role: "assistant".to_string(),
            content: answers.join(""),
        });

        // Assuming `sd_image` is true and `sd(sd_config)` returns `Result<Vec<Vec<u8>>>`
        if sd_image {
            let mut sd_config = SDConfig::new();
            sd_config.prompt = answers.join("");
            sd_config.height = Some(512);
            sd_config.width = Some(512);

            let images_result = sd(sd_config); // This call now returns `Result<Vec<Vec<u8>>>`

            match images_result {
                Ok(images) => {
                    // Send images over NDI
                    if ndi_images {
                        debug!("Sending images over NDI");
                    }
                    #[cfg(feature = "ndi")]
                    if ndi_images {
                        send_images_over_ndi(images.clone())?;
                    }

                    // Save images to disk
                    for (index, image_bytes) in images.iter().enumerate() {
                        let image_file = format!("{}.png", index);
                        println!("Image {} saving to {}", index + 1, image_file);
                        image_bytes
                            .save(image_file)
                            .map_err(candle_core::Error::wrap)?;
                    }
                }
                Err(e) => eprintln!("Error generating images: {:?}", e),
            }
        }
    }

    // After processing all chunks/responses
    Ok(response_messages) // Return the collected messages
}
