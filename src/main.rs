use reqwest::Client;
use std::env;
use tokio;
use serde_derive::{Serialize, Deserialize};
use serde_json;

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
    total_tokens: i32
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
    message: Message
}

/*
 * {"choices":[{"finish_reason":"stop","index":0,"message":{"content":"The Los Angeles Dodgers won
 * the World Series in 2020. They defeated the Tampa Bay Rays in six
 * games.","role":"assistant"}}],"created":1706900958,"id":"chatcmpl-8jqjxqYj1IkKixqlHVvmTyJynoPOjaoA","model":"gpt-3.5-turbo","object":"chat.completion","usage":{"completion_tokens":30,"prompt_tokens":62,"total_tokens":92}}
 */

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
