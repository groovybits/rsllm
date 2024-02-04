use reqwest::Client;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::env;
use tokio;

const OPENAI_ENDPOINT: &str = "http://earth.groovylife.ai:8081/v1/chat/completions";

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
}

#[derive(Deserialize)]
struct Completions {
    completion_tokens: i32,
    prompt_tokens: i32,
    total_tokens: i32,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    created: i64,
    id: String,
    model: String,
    object: String,
    usage: Completions,
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    finish_reason: String,
    index: i32,
    message: Message,
}

/*
 * {"choices":[{"finish_reason":"stop","index":0,"message":{"content":"The Los Angeles Dodgers won
 * the World Series in 2020. They defeated the Tampa Bay Rays in six
 * games.","role":"assistant"}}],"created":1706900958,"id":"chatcmpl-8jqjxqYj1IkKixqlHVvmTyJynoPOjaoA","model":"gpt-3.5-turbo","object":"chat.completion","usage":{"completion_tokens":30,"prompt_tokens":62,"total_tokens":92}}
 */

#[tokio::main]
async fn main() {
    let openai_key =
        env::var("OPENAI_API_KEY").unwrap_or_else(|_| panic!("OPENAI_API_KEY not set in env"));

    let client = Client::new();

    let packet_dump = "analyze this mpegts nal dump and packet information:
    --- Packet Offset 0 Packet Length 88 ---

    0000: 00 00 01 01 9f 70 74 41 9f 00 02 a6 82 1d 76 1b
    0010: 69 92 36 f1 8c fb a9 87 5a 48 68 5d 5d bd 58 75
    0020: 6d fd f5 32 d6 9d dc 88 b1 97 d0 40 79 39 f0 ea
    0030: f0 b1 61 34 c4 2e d1 b1 ab f5 95 c5 b6 20 58 bb
    0040: e8 95 f5 22 86 88 bc 09 7b 79 0e fe c1 81 14 85
    0050: 9a 26 9f 58 d4 cc 1e 2e
    ---";

    let system_message = Message {
        role: "system".to_string(),
        content: "You are a helpful assistant.".to_string(),
    };
    let user_message = Message {
        role: "user".to_string(),
        content: packet_dump.to_string(),
    };
    let messages = vec![system_message, user_message];

    let resp = client
        .post(OPENAI_ENDPOINT)
        .header("Authorization", format!("Bearer {}", openai_key))
        .json(&OpenAIRequest {
            model: "gpt-3.5-turbo",
            messages: &messages,
        })
        .send()
        .await
        .unwrap_or_else(|err| {
            println!("Failed to send request: {}", err);
            std::process::exit(1);
        })
        .text() // get the full response text
        .await
        .unwrap_or_else(|err| {
            println!("Failed to read response text: {}", err);
            std::process::exit(1);
        });

    let response: Result<OpenAIResponse, _> = serde_json::from_str(&resp);

    match response {
        Ok(res) => {
            println!("Finished because: {}", res.choices[0].finish_reason);
            println!("Response: {}", res.choices[0].message.content);
        }
        Err(e) => {
            // Print the error and the response that caused it
            println!("Failed to parse response: {}", e);
            println!("Response that failed to parse: {}", resp);
        }
    }
}
