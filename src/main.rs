/*
 * RsLLM OpenAI API client
 * This program is a simple client for the OpenAI API. It sends a prompt to the API and prints the
 * response to the console.
 * The program is written in Rust and uses the reqwest crate for making HTTP requests.
 * The program uses the clap crate for parsing command line arguments.
 * The program uses the serde and serde_json crates for serializing and deserializing JSON.
 * The program uses the log crate for logging.
 * The program uses the tokio crate for asynchronous IO.
 * The program uses the chrono crate for working with dates and times.
 * The program uses the dotenv crate for reading environment variables from a .env file.
 *
 * Chris Kennedy (C) February 2024
 * MIT License
 *
*/

use bytes::Bytes;
use chrono::NaiveDateTime;
use clap::Parser;
use log::{debug, error, info};
use reqwest::Client;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::env;
use std::io::Write;
use std::time::Instant;
use tokio::sync::mpsc;

/// RScap Probe Configuration
#[derive(Parser, Debug)]
#[clap(
    author = "Chris Kennedy",
    version = "1.0",
    about = "RsLLM OpenAI API client"
)]
struct Args {
    /// System prompt
    #[clap(
        long,
        env = "SYSTEM_PROMPT",
        default_value = "You are an assistant who can do anything that is asked of you to help and assist in any way possible. Always be polite and respectful, take ownership and responsibility for the tasks requested of you, and make sure you complete them to the best of your ability.
        When coding product complete examples of production grade fully ready to run code."
    )]
    system_prompt: String,

    /// System prompt
    #[clap(
        long,
        env = "QUERY",
        default_value = "Explain each MpegTS NAL type in a chart format.",
        help = "Query to generate completions for"
    )]
    query: String,

    /// Temperature
    #[clap(
        long,
        env = "TEMPERATURE",
        default_value = "0.8",
        help = "Temperature for LLM sampling, 0.0 to 1.0, it will cause the LLM to generate more random outputs. 0.0 is deterministic, 1.0 is maximum randomness. Default is 0.8."
    )]
    temperature: f32,

    /// Top P
    #[clap(long, env = "TOP_P", default_value = "1.0", help = "Top P")]
    top_p: f32,

    /// Presence Penalty
    #[clap(
        long,
        env = "PRESENCE_PENALTY",
        default_value = "0.0",
        help = "Presence Penalty, it will cause the LLM to generate more diverse outputs. 0.0 is deterministic, 1.0 is maximum randomness. Default is 0.0."
    )]
    presence_penalty: f32,

    /// Frequency Penalty
    #[clap(
        long,
        env = "FREQUENCY_PENALTY",
        default_value = "0.0",
        help = "Frequency Penalty, it will cause the LLM to generate more diverse outputs. 0.0 is deterministic, 1.0 is maximum randomness. Default is 0.0."
    )]
    frequency_penalty: f32,

    /// Max Tokens
    #[clap(
        long,
        env = "MAX_TOKENS",
        default_value = "2000",
        help = "Max Tokens, 1 to 4096. Default is 2000."
    )]
    max_tokens: i32,

    /// Model
    #[clap(
        long,
        env = "MODEL",
        default_value = "gpt-4-0125-preview",
        help = "OpenAI LLM Model (N/A with local Llama2 based LLM)"
    )]
    model: String,

    /// LLM Host url with protocol, host, port,  no path
    #[clap(
        long,
        env = "LLM_HOST",
        default_value = "http://127.0.0.1:8080",
        help = "LLM Host url with protocol, host, port,  no path"
    )]
    llm_host: String,

    /// LLM Url path
    #[clap(
        long,
        env = "LLM_PATH",
        default_value = "/v1/chat/completions",
        help = "LLM Url path for completions, default is /v1/chat/completions."
    )]
    llm_path: String,

    /// Don't stream output
    #[clap(
        long,
        env = "NO_STREAM",
        default_value = "false",
        help = "Don't stream output, wait for all completions to be generated before returning. Default is false."
    )]
    no_stream: bool,

    /// Safety feature for using openai api and confirming you understand the risks
    #[clap(
        long,
        env = "USE_OPENAI",
        default_value = "false",
        help = "Safety feature for using openai api and confirming you understand the risks, you must also set the OPENAI_API_KEY, this will set the llm-host to api.openai.com. Default is false."
    )]
    use_openai: bool,

    /// debug inline on output (can mess up the output) as a bool
    #[clap(
        long,
        env = "DEBUG_INLINE",
        default_value = "false",
        help = "debug inline on output (can mess up the output) as a bool. Default is false."
    )]
    debug_inline: bool,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    max_tokens: &'a i32,        // add this field to the request struct
    temperature: &'a f32,       // add this field to the request struct
    top_p: &'a f32,             // add this field to the request struct
    presence_penalty: &'a f32,  // add this field to the request struct
    frequency_penalty: &'a f32, // add this field to the request struct
    stream: &'a bool,
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
struct Choice {
    finish_reason: Option<String>,
    logprobs: Option<bool>,
    index: i32,
    delta: Delta, // Use Option to handle cases where it might be null or missing
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

/*
 * {"choices":[{"finish_reason":"stop","index":0,"message":{"content":"The Los Angeles Dodgers won
 * the World Series in 2020. They defeated the Tampa Bay Rays in six
 * games.","role":"assistant"}}],"created":1706900958,"id":"chatcmpl-8jqjxqYj1IkKixqlHVvmTyJynoPOjaoA","model":"gpt-3.5-turbo","object":"chat.completion","usage":{"completion_tokens":30,"prompt_tokens":62,"total_tokens":92}}
 */

async fn stream_completion(
    open_ai_request: OpenAIRequest<'_>,
    openai_key: &str,
    llm_host: &str,
    llm_path: &str,
    debug_inline: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

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
    let mut loop_count = 0;
    // errors are strings

    println!("\nResponse status: {}\n---\n", response.status());
    debug!("Headers: {:#?}\n---\n", response.headers());
    if !open_ai_request.stream {
        println!("Body: {}\n---\n", response.text().await?);
    } else {
        // Create an mpsc channel
        let (tx, mut rx) = mpsc::channel::<Bytes>(32);
        let (etx, mut erx) = mpsc::channel::<String>(32);

        // loop through the chunks
        // Spawn a new task for each chunk to process it asynchronously
        let worker = tokio::spawn(async move {
            while let Some(chunk) = rx.recv().await {
                loop_count += 1;

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
                                    let duration = end_time.duration_since(start_time);
                                    let pretty_time = format!("{:?}", duration);

                                    println!(
                                        "\n--\nIndex {} ID {}\nObject {} by Model {} User {}\nCreated on {} Finish reason: {}\nTokens {} Bytes {} at {} tokens per second and {} seconds to complete.\n--\n",
                                        choice.index,
                                        id,
                                        object,
                                        model,
                                        role,
                                        created_date,
                                        reason,
                                        token_count,
                                        byte_count,
                                        token_count / duration.as_secs(),
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
                                    print!("{}", content);
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
                                etx.send(format!("{} - {}", e, response_json))
                                    .await
                                    .expect("Failed to send error");
                                print!("*X*");
                            }
                        }
                    }
                }
            }
        });

        // Spawn a separate task to collect errors concurrently
        let error_collector = tokio::spawn(async move {
            let mut errors = Vec::new();
            while let Some(error_message) = erx.recv().await {
                errors.push(error_message);
            }
            errors // Return collected errors from the task
        });

        // Main task to send chunks to the worker
        while let Some(chunk) = response.chunk().await? {
            tx.send(chunk).await.expect("Failed to send chunk");
        }

        // Close the channel by dropping tx
        drop(tx);

        // Await the worker task to finish processing
        worker.await?;

        // Await the error collector task to retrieve the collected errors
        let errors_array = error_collector.await?; // Handle errors from the error collector task

        // Print errors or perform further actions with them
        if !errors_array.is_empty() {
            println!("\nErrors:");
            for error in errors_array.iter() {
                println!("{}", error);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok(); // read .env file
                           // Initialize logging
    let _ = env_logger::try_init();

    let args = Args::parse();

    let openai_key = env::var("OPENAI_API_KEY")
        .ok()
        .unwrap_or_else(|| "NO_API_KEY".to_string());
    let system_prompt = args.system_prompt;
    let query = args.query;

    let system_message = Message {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    };
    let user_message = Message {
        role: "user".to_string(),
        content: query.to_string(),
    };
    let messages = vec![system_message, user_message];

    // add these values to the input for completions endpoint
    let temperature = args.temperature;
    let top_p = args.top_p;
    let presence_penalty = args.presence_penalty;
    let frequency_penalty = args.frequency_penalty;
    let max_tokens = args.max_tokens;
    let model = args.model;
    let mut llm_host = args.llm_host;
    let llm_path = args.llm_path;
    let debug_inline = args.debug_inline;

    if args.use_openai {
        // set the llm_host to the openai api
        llm_host = "https://api.openai.com".to_string();
    }

    // Stream API Completion
    let stream = !args.no_stream;
    let open_ai_request = OpenAIRequest {
        model: &model,
        max_tokens: &max_tokens, // add this field to the request struct
        messages: &messages,
        temperature: &temperature, // add this field to the request struct
        top_p: &top_p,             // add this field to the request struct
        presence_penalty: &presence_penalty, // add this field to the request struct
        frequency_penalty: &frequency_penalty, // add this field to the request struct
        stream: &stream,
    };

    // Directly await the future; no need for an explicit runtime block
    stream_completion(
        open_ai_request,
        &openai_key,
        &llm_host,
        &llm_path,
        debug_inline,
    )
    .await
    .unwrap();
}
