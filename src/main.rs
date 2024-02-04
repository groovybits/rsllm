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

use chrono::NaiveDateTime;
use clap::Parser;
use log::{debug, error};
use reqwest::Client;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::io::Write;
use tokio; // Import traits and modules required for IO operations

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
        default_value = "You are an assistant who is helpful."
    )]
    system_prompt: String,

    /// System prompt
    #[clap(
        long,
        env = "QUERY",
        default_value = "analyze this mpegts nal dump and packet information, give a short summary of the stats like an mpegts
        analyzer would do. The nal dump is as follows:

    --- Packet Offset 0 Packet Length 88 ---

    0000: 00 00 01 01 9f 70 74 41 9f 00 02 a6 82 1d 76 1b
    0010: 69 92 36 f1 8c fb a9 87 5a 48 68 5d 5d bd 58 75
    0020: 6d fd f5 32 d6 9d dc 88 b1 97 d0 40 79 39 f0 ea
    0030: f0 b1 61 34 c4 2e d1 b1 ab f5 95 c5 b6 20 58 bb
    0040: e8 95 f5 22 86 88 bc 09 7b 79 0e fe c1 81 14 85
    0050: 9a 26 9f 58 d4 cc 1e 2e
    ---"
    )]
    query: String,

    /// Temperature
    #[clap(long, env = "TEMPERATURE", default_value = "0.8")]
    temperature: f32,

    /// Top P
    #[clap(long, env = "TOP_P", default_value = "1.0")]
    top_p: f32,

    /// Presence Penalty
    #[clap(long, env = "PRESENCE_PENALTY", default_value = "0.0")]
    presence_penalty: f32,

    /// Frequency Penalty
    #[clap(long, env = "FREQUENCY_PENALTY", default_value = "0.0")]
    frequency_penalty: f32,

    /// Max Tokens
    #[clap(long, env = "MAX_TOKENS", default_value = "800")]
    max_tokens: i32,

    /// Stream
    #[clap(long, env = "STREAM", default_value = "true")]
    stream: bool,

    /// Model
    #[clap(long, env = "MODEL", default_value = "gpt-3.5-turbo")]
    model: String,

    /// OpenAI API Key
    #[clap(long, env = "OPENAI_API_KEY", default_value = "FAKE_KEY")]
    openai_key: String,

    /// LLM Host url with protocol, host, port,  no path
    #[clap(
        long,
        env = "LLM_HOST",
        default_value = "http://earth.groovylife.ai:8081"
    )]
    llm_host: String,

    /// LLM Url path
    #[clap(long, env = "LLM_PATH", default_value = "/v1/chat/completions")]
    llm_path: String,
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
    created: i64,
    id: String,
    model: String,
    object: String,
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    finish_reason: Option<String>,
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
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let response_result = client
        .post(format!("{}{}", llm_host, llm_path))
        .header("Authorization", format!("Bearer {}", openai_key))
        .json(&open_ai_request)
        .send()
        .await;

    // Handle response_result error properly
    if response_result.is_err() {
        error!("Failed to send request: {}", response_result.unwrap_err());
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Network request failed",
        )));
    }

    let mut response = response_result.unwrap(); // this is safe due to the check above
    let mut token_count = 0;
    let mut byte_count = 0;

    println!("\nResponse status: {}\n---\n", response.status());
    while let Ok(Some(chunk)) = response.chunk().await {
        let mut accumulated_response = Vec::new();
        for byte in &chunk {
            accumulated_response.push(*byte);
        }
        /* Example of a response chunk string we need to turn into a openairesponse struct
        data: {"choices":[{"delta":{"content":"."},"finish_reason":null,"index":0}],"created":1707049435,"id":"chatcmpl-VAvCRGJHvO9SZYJ4ycqgG99tNshaWbgC","model":"gpt-3.5-turbo","object":"chat.completion.chunk"}
        data: {"choices":[{"delta":{},"finish_reason":"stop","index":0}],"created":1707049435,"id":"chatcmpl-mB6KoI6xFxkiDtVovFtPrBh8BD2sgC2G","model":"gpt-3.5-turbo","object":"chat.completion.chunk"}
        */
        let removed_data = accumulated_response[6..].to_vec();
        let response_json = String::from_utf8(removed_data)?;
        debug!("Final response: {}", response_json);

        match serde_json::from_str::<OpenAIResponse>(&response_json) {
            Ok(res) => match res.choices.get(0) {
                Some(choice) => {
                    // check if we have a finish reason
                    if let Some(reason) = &choice.finish_reason {
                        println!(
                            "\n--\nIndex {} ID {}\nObject {} by Model {}\nCreated on {} Finish reason: {}\nTokens {} Bytes {}\n--\n",
                            choice.index,
                            res.id,
                            res.object,
                            res.model,
                            NaiveDateTime::from_timestamp_opt(res.created, 0).unwrap(),
                            reason,
                            token_count,
                            byte_count
                        );
                        break; // break the loop if we have a finish reason
                    }

                    // check if we have content in the delta
                    if let Some(content) = &choice.delta.content {
                        token_count += 1;
                        byte_count += content.len();
                        print!("{}", content);
                        // flush stdout
                        std::io::stdout().flush().unwrap();
                    }
                }
                None => error!("No choices available."),
            },
            Err(e) => {
                // Handle the parse error here
                error!("Failed to parse response: {}", e);
                error!("Response that failed to parse: {}", response_json);
            }
        }
    }
    println!();

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok(); // read .env file

    let args = Args::parse();

    let openai_key = args.openai_key;
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
    let stream = args.stream;
    let model = args.model;
    let llm_host = args.llm_host;
    let llm_path = args.llm_path;

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
    stream_completion(open_ai_request, &openai_key, &llm_host, &llm_path)
        .await
        .unwrap();
}
