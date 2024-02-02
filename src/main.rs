use reqwest::Client;
use std::env;
use tokio;
use serde_derive::{Serialize, Deserialize};
use serde_json;

const OPENAI_ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

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
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    finish_reason: String,
    model: String,
    created: i64,
    message: Message
}

#[tokio::main]
async fn main() {
    let openai_key = env::var("OPENAI_API_KEY").unwrap_or_else(|_| panic!("OPENAI_API_KEY not set in env"));

    let client = Client::new();

    let system_message = Message { role: "system".to_string(), content: "You are a helpful assistant.".to_string() };
    let user_message = Message { role: "user".to_string(), content: "Who won the world series in 2020?".to_string() };
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

    let response : Result<OpenAIResponse, _> = serde_json::from_str(&resp);

    match response {
        Ok(res) => {
            println!("Finished because: {}", res.choices[0].finish_reason);
            println!("Response: {}", res.choices[0].message.content);
        },
        Err(e) => {
            // Print the error and the response that caused it
            println!("Failed to parse response: {}", e);
            println!("Response that failed to parse: {}", resp);
        }
    }
}
