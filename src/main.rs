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
use log::{debug, error, info};
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
        default_value = "You are a broadcast engineer who is an expert at handling mpegts video packets,
            having the ability to parse them realtime by looking through them.
            provide mpegts analyzer output at a professional broadcast equipment level."
    )]
    system_prompt: String,

    /// System prompt
    #[clap(
        long,
        env = "QUERY",
        default_value = "analyze this mpegts nal dump and packet information,
        give a chart showing the packet sections information decoded for nal packets and other stats like an mpegts
        analyzer would do.

        The video settings for this stream are:
        - ffmpeg -f lavfi -i smptebars=size=1920x1080:rate=29.976 -f lavfi -i sine=frequency=1000:sample_rate=48000 \
               -c:v libx264 -c:a aac -b:a 128k -ar 48000 -ac 2 \
               -mpegts_pmt_start_pid 0x1000 -mpegts_start_pid 0x0100 \
               -metadata service_provider=TestStream -metadata service_name=ColorBarsWithTone \
               -nal-hrd cbr -maxrate 19M -minrate 19M -bufsize 19M -b:v 60M -muxrate 20M

        The nal dump is as follows:

        0000: 47 01 00 10 0d a9 6f 55 b2 e5 06 63 1f 95 7e 4c
        0010: a9 78 ab b3 73 b5 11 0b 9d dd 40 8f 3f 9c 32 75
        0020: 89 47 64 45 99 76 a9 a2 68 97 75 d8 05 42 e4 f8
        0030: 95 6a 49 51 61 a8 09 9c bb 29 bb 71 b8 70 6d 21
        0040: bd 43 8a 0f 05 e6 79 f9 bd d5 af 85 05 e1 ff 0d
        0050: c5 ce 53 97 89 9a 7b 06 2b 74 f0 87 16 93 6d 9e
        0060: 41 f0 cc 3b f5 6f 7c 14 9d 25 75 ab b7 c5 b8 9a
        0070: cd 10 06 9a 30 48 49 66 6c cc 20 6f ab e5 22 6a
        0080: d7 6a 96 25 03 c5 a6 bb 9d aa 9a 93 17 8d 44 c4
        0090: 94 7f 02 e7 c0 6d dd b5 1a 66 d3 9d 08 4e 6e b8
        00a0: 47 d6 a5 fd 1f ff c8 41 8a 90 e9 d0 3c 5c ef 8c
        00b0: 9c 71 d6 e1 82 5a c0 da 74 dc c7 52

        0000: 47 01 00 11 ac 00 1f 25 4c d5 bb 3c 0a 69 9c a3
        0010: da e7 a9 07 37 2b e4 fb cb 1b e4 77 ca 23 8e d0
        0020: 9b 8c ba 4c 1d a9 f2 d1 0e b7 7f f4 73 37 cf 7d
        0030: 78 34 97 05 fd 80 14 fb 9a 1a 39 1a 3e 75 6d 7b
        0040: be 0a ae 3b 86 3c 89 a0 63 e5 4b d7 8f 58 4c c6
        0050: cb 17 13 e6 85 09 a9 69 e5 58 11 a4 a5 8b 18 cd
        0060: 91 42 f0 c6 6c 2a 93 c0 9d f5 08 f4 1d 4b 89 26
        0070: f2 aa d6 8b 40 a1 da 36 c5 da 88 29 4c 14 30 5f
        0080: 91 4a 0b 0f 94 5e b2 29 de fd 99 ed e6 63 2d 98
        0090: da 5c 72 32 fb ae 06 90 9d 4f 9f 28 ee 8f 3a 7b
        00a0: 04 6a aa 54 8f e2 9b d0 f9 40 5c b4 a3 be 5a dd
        00b0: b8 cc 9a 37 f7 50 76 29 12 0a 7d 50

        0000: 47 01 00 12 eb 76 7c 60 92 c8 f5 2b 3e 17 e2 21
        0010: 72 07 43 83 75 10 21 bb 11 d8 31 1c 1c 80 a6 7c
        0020: c2 27 be 43 72 9c 33 55 48 61 0d 04 9e fd 56 7b
        0030: c1 9b d7 5d 94 39 ce 81 5e 29 41 31 15 84 1d a3
        0040: f7 79 1e 27 5a f9 d1 dc 71 2c a3 e0 e7 d3 be a0
        0050: 94 38 ea 71 87 fc 0f 75 f6 ef 03 5f 42 15 8c 8f
        0060: ea 75 e8 c1 55 fd ee 46 40 aa a9 db 2a dd 81 5c
        0070: 4d 74 97 f1 49 c0 af e9 0c 6b 17 94 81 a2 c5 00
        0080: c4 f1 29 62 52 54 2d c0 9a 6f f9 ac fe aa 8b 44
        0090: b0 40 65 cc f3 1c 2f 11 81 14 d7 fd af 89 6d 1a
        00a0: f2 f5 6a dc 08 29 41 13 38 c9 86 1f c3 49 b1 5c
        00b0: 76 b2 53 39 5d d2 89 92 d9 bf b7 44

        0000: 47 01 00 13 a3 ed 45 59 74 9a f1 d1 66 31 4e 1a
        0010: f5 94 67 cc 11 1f e6 cc e7 e0 d7 91 54 ab c0 71
        0020: aa 2e 16 19 32 1b ca 16 50 4d 88 06 47 7d 43 a0
        0030: df 70 a7 ff 6e b6 88 c3 ac 72 0a 05 98 90 0d 66
        0040: cf 6b 61 95 ec 9f b3 06 3e a6 e5 99 ba c5 b8 a3
        0050: 54 86 dc c5 48 d6 eb 07 84 58 93 07 59 11 06 5d
        0060: d0 12 4d 11 f5 8a ed 5d 8b 89 72 e5 16 c3 51 3d
        0070: 24 68 2c 85 dd ff ff ec d0 3b 94 fc e6 6a 40 e3
        0080: 85 fd ac 42 5f 6d 53 2a 07 24 7d 49 dc 31 33 7f
        0090: b0 e1 23 37 27 e5 d4 76 e3 b8 01 2e ff fd 97 90
        00a0: 42 31 e6 2b b8 57 f5 da cd 3a d3 3e fb b2 1b 82
        00b0: 78 42 43 8f 2c 7c 82 8d 51 10 b6 8d

        "
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

    /// Model
    #[clap(long, env = "MODEL", default_value = "gpt-4-0125-preview")]
    model: String,

    /// OpenAI API Key
    #[clap(long, env = "OPENAI_API_KEY", default_value = "ADD_YOUR_KEY_TO_ENV")]
    openai_key: String,

    /// LLM Host url with protocol, host, port,  no path
    #[clap(long, env = "LLM_HOST", default_value = "https://api.openai.com")]
    llm_host: String,

    /// LLM Url path
    #[clap(long, env = "LLM_PATH", default_value = "/v1/chat/completions")]
    llm_path: String,

    /// Don't stream output
    #[clap(long, env = "NO_STREAM", default_value = "false")]
    no_stream: bool,
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
    let mut loop_count = 0;

    println!("\nResponse status: {}\n---\n", response.status());
    debug!("Headers: {:#?}\n---\n", response.headers());
    if !open_ai_request.stream {
        println!("Body: {}\n---\n", response.text().await?);
    } else {
        // check if we got a response with chunks
        if response.chunk().await.is_err() {
            error!("Failed to get response chunks");
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to get response chunks",
            )));
        }

        // loop through the chunks
        while let Ok(Some(chunk)) = response.chunk().await {
            loop_count += 1;
            debug!("#{} LLM Result Chunk: {:#?}\n", loop_count, chunk);
            let mut accumulated_response = Vec::new();
            for byte in &chunk {
                accumulated_response.push(*byte);
            }
            /* Example of a response chunk string we need to turn into a openairesponse struct
            data: {"choices":[{"delta":{"content":"."},"finish_reason":null,"index":0}],"created":1707049435,"id":"chatcmpl-VAvCRGJHvO9SZYJ4ycqgG99tNshaWbgC","model":"gpt-3.5-turbo","object":"chat.completion.chunk"}
            data: {"choices":[{"delta":{},"finish_reason":"stop","index":0}],"created":1707049435,"id":"chatcmpl-mB6KoI6xFxkiDtVovFtPrBh8BD2sgC2G","model":"gpt-3.5-turbo","object":"chat.completion.chunk"}
            */

            // check for [DONE] as the response after 'data: ' like 'data: [DONE]\n' as OpenAI sends
            if accumulated_response.len() >= 6
                && accumulated_response[6..] == [91, 68, 79, 78, 69, 93, 10]
            {
                info!("End of response chunks.\n");
                break;
            }

            if accumulated_response.len() < 6 {
                if accumulated_response == [10] {
                    debug!("Empty line in response chunks.");
                }
                if accumulated_response.len() == 0 {
                    debug!("Empty line in response chunks.");
                } else {
                    error!("Invalid response chunk:\n - '{:?}'", accumulated_response);
                }
                continue;
            }
            let mut offset = 0;
            // check if accumulated_response starts with 'data: ' and if so change offset to 6
            if accumulated_response[0..6] == [100, 97, 116, 97, 58, 32] {
                offset = 6;
            }
            let removed_data = accumulated_response[offset..].to_vec();
            let response_json = String::from_utf8(removed_data)?;
            debug!("Chunk #{} response: {}", loop_count, response_json);

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

                        // check for system_fingerprint
                        if let Some(fingerprint) = &res.system_fingerprint {
                            println!("System fingerprint: {}", fingerprint);
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
    }
    println!();

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok(); // read .env file
                           // Initialize logging
    let _ = env_logger::try_init();

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
    let model = args.model;
    let llm_host = args.llm_host;
    let llm_path = args.llm_path;

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
    stream_completion(open_ai_request, &openai_key, &llm_host, &llm_path)
        .await
        .unwrap();
}
