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
use rsllm::network_capture::{network_capture, NetworkCapture};
use rsllm::stream_data::{
    get_pid_map, identify_video_pid, is_mpegts_or_smpte2110, parse_and_store_pat, process_packet,
    update_pid_map, Codec, PmtInfo, StreamData, Tr101290Errors, PAT_PID,
};
use rsllm::stream_data::{process_mpegts_packet, process_smpte2110_packet};
use rsllm::{current_unix_timestamp_ms, hexdump, hexdump_ascii};
use rsllm::{get_stats_as_json, StatsType};
use serde_derive::{Deserialize, Serialize};
use serde_json::{self, json};
use std::env;
use std::io;
use std::io::Write;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tokio::sync::mpsc::{self};
use tokio::time::Duration;

/// RScap Probe Configuration
#[derive(Parser, Debug)]
#[clap(
    author = "Chris Kennedy",
    version = "1.1",
    about = "Rust LLM - AI System/Network/Stream Analyzer"
)]
struct Args {
    /// System prompt
    #[clap(
        long,
        env = "SYSTEM_PROMPT",
        default_value = "You will recieve data in the prompt to analzye. You are able to say green or red depending on the data streams health determined from various forms of analysis as needed. The data is either system os stats or mpegts packets, you will know by the format and content which it is.",
        help = "System prompt"
    )]
    system_prompt: String,

    /// System prompt
    #[clap(
        long,
        env = "QUERY",
        default_value = "Determine if the stream is healthy or sick, diagnose the issue if possible or give details about it. Use the historical view to see bigger trends of the stream of data shown above. It will be in older to newer order per sample period shown by the timestamps per period.",
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
    #[clap(
        long,
        env = "TOP_P",
        default_value = "1.0",
        help = "Top P sampling, 0.0 to 1.0. Default is 1.0."
    )]
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
        default_value = "800",
        help = "Max Tokens, 1 to N. Default is 800."
    )]
    max_tokens: i32,

    /// Model
    #[clap(
        long,
        env = "MODEL",
        default_value = "no-model-specified",
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

    /// LLM History size
    #[clap(
        long,
        env = "LLM_HISTORY_SIZE",
        default_value = "16768",
        help = "LLM History size, default is 16768 (0 is unlimited)."
    )]
    llm_history_size: usize,

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

    /// Show output errors
    #[clap(
        long,
        env = "SHOW_OUTPUT_ERRORS",
        default_value = "false",
        help = "Show LLM output errors which may mess up the output and niceness if packet loss occurs, default is false."
    )]
    show_output_errors: bool,

    /// Monitor system stats
    #[clap(
        long,
        env = "AI_OS_STATS",
        default_value = "false",
        help = "Monitor system stats, default is false."
    )]
    ai_os_stats: bool,

    /// run as a daemon monitoring the specified stats
    #[clap(
        long,
        env = "DAEMON",
        default_value = "false",
        help = "run as a daemon monitoring the specified stats, default is false."
    )]
    daemon: bool,

    /// AI Network Stats
    #[clap(
        long,
        env = "AI_NETWORK_STATS",
        default_value = "false",
        help = "Monitor network stats, default is false."
    )]
    ai_network_stats: bool,

    /// AI Network Packets - also send all the packets not jsut the pidmap stats
    #[clap(
        long,
        env = "AI_NETWORK_PACKETS",
        default_value = "false",
        help = "Monitor network packets, default is false."
    )]
    ai_network_packets: bool,

    /// AI Network Full Packet Hex Dump
    #[clap(
        long,
        env = "AI_NETWORK_HEXDUMP",
        default_value = "false",
        help = "Monitor network full packet hex dump, default is false."
    )]
    ai_network_hexdump: bool,

    /// AI Network Packet Count
    #[clap(
        long,
        env = "AI_NETWORK_PACKET_COUNT",
        default_value_t = 100,
        help = "AI Network Packet Count, default is 100."
    )]
    ai_network_packet_count: usize,

    /// PCAP output capture stats mode
    #[clap(
        long,
        env = "PCAP_STATS",
        default_value_t = false,
        help = "PCAP output capture stats mode, default is false."
    )]
    pcap_stats: bool,

    /// Sets the batch size
    #[clap(
        long,
        env = "PCAP_BATCH_SIZE",
        default_value_t = 7,
        help = "Sets the batch size, default is 7."
    )]
    pcap_batch_size: usize,

    /// Sets the payload offset
    #[clap(
        long,
        env = "PAYLOAD_OFFSET",
        default_value_t = 42,
        help = "Sets the payload offset, default is 42."
    )]
    payload_offset: usize,

    /// Sets the packet size
    #[clap(
        long,
        env = "PACKET_SIZE",
        default_value_t = 188,
        help = "Sets the packet size, default is 188."
    )]
    packet_size: usize,

    /// Sets the pcap buffer size
    #[clap(long, env = "BUFFER_SIZE", default_value_t = 1 * 1_358 * 1_000, help = "Sets the pcap buffer size, default is 1 * 1_358 * 1_000.")]
    buffer_size: i64,

    /// Sets the read timeout
    #[clap(
        long,
        env = "READ_TIME_OUT",
        default_value_t = 300_000,
        help = "Sets the read timeout, default is 60_000."
    )]
    read_time_out: i32,

    /// Sets the source device
    #[clap(
        long,
        env = "SOURCE_DEVICE",
        default_value = "",
        help = "Sets the source device for pcap capture."
    )]
    source_device: String,

    /// Sets the source IP
    #[clap(
        long,
        env = "SOURCE_IP",
        default_value = "224.0.0.200",
        help = "Sets the source IP to capture for pcap."
    )]
    source_ip: String,

    /// Sets the source protocol
    #[clap(
        long,
        env = "SOURCE_PROTOCOL",
        default_value = "udp",
        help = "Sets the source protocol to capture for pcap."
    )]
    source_protocol: String,

    /// Sets the source port
    #[clap(
        long,
        env = "SOURCE_PORT",
        default_value_t = 10_000,
        help = "Sets the source port to capture for pcap, default is 10000."
    )]
    source_port: i32,

    /// Sets if wireless is used
    #[clap(
        long,
        env = "USE_WIRELESS",
        default_value_t = false,
        help = "Sets if wireless is used, default is false."
    )]
    use_wireless: bool,

    /// Use promiscuous mode
    #[clap(
        long,
        env = "PROMISCUOUS",
        default_value_t = false,
        help = "Use promiscuous mode for network capture, default is false."
    )]
    promiscuous: bool,

    /// PCAP immediate mode
    #[clap(
        long,
        env = "IMMEDIATE_MODE",
        default_value_t = false,
        help = "PCAP immediate mode, default is false."
    )]
    immediate_mode: bool,

    /// Hexdump
    #[clap(
        long,
        env = "HEXDUMP",
        default_value_t = false,
        help = "Hexdump mpegTS packets, default is false."
    )]
    hexdump: bool,

    /// Show the TR101290 p1, p2 and p3 errors if any
    #[clap(
        long,
        env = "SHOW_TR101290",
        default_value_t = false,
        help = "Show the TR101290 p1, p2 and p3 errors if any, default is false."
    )]
    show_tr101290: bool,

    /// PCAP Channel Size, drop packets if channel is full, 1g = 1_000_000
    #[clap(
        long,
        env = "PCAP_CHANNEL_SIZE",
        default_value_t = 1_000_000,
        help = "PCAP Channel Size, drop packets if channel is full, 1g = 1_000_000."
    )]
    pcap_channel_size: usize,

    /// DEBUG LLM Message History
    #[clap(
        long,
        env = "DEBUG_LLM_HISTORY",
        default_value_t = false,
        help = "DEBUG LLM Message History, default is false."
    )]
    debug_llm_history: bool,

    /// POLL Interval in ms, default to 300 seconds
    #[clap(
        long,
        env = "POLL_INTERVAL",
        default_value_t = 300_000,
        help = "POLL Interval in ms, default to 5 minutes or 300 seconds."
    )]
    poll_interval: u64,

    /// Turn off progress output dots
    #[clap(
        long,
        env = "NO_PROGRESS",
        default_value_t = false,
        help = "Turn off progress output dots, default is false."
    )]
    no_progress: bool,

    /// Loglevel, control rust log level
    #[clap(
        long,
        env = "LOGLEVEL",
        default_value = "",
        help = "Loglevel, control rust log level, default is info."
    )]
    loglevel: String,

    /// Break Line Length - line length for breaking lines from LLM messages, default is 120
    #[clap(
        long,
        env = "BREAK_LINE_LENGTH",
        default_value_t = 120,
        help = "Break Line Length - line length for breaking lines from LLM messages, default is 120."
    )]
    break_line_length: usize,
}

#[derive(Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: &'a str,
    messages: Vec<Message>,
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

async fn stream_completion(
    open_ai_request: OpenAIRequest<'_>,
    openai_key: &str,
    llm_host: &str,
    llm_path: &str,
    debug_inline: bool,
    show_output_errors: bool,
    break_line_length: usize,
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

                                    println!(
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
    }

    // After processing all chunks/responses
    Ok(response_messages) // Return the collected messages
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok(); // read .env file
                           // Initialize logging
    let _ = env_logger::try_init();

    let args = Args::parse();

    // set Rust log level with --loglevel if it is set
    let loglevel = args.loglevel.to_lowercase();
    match loglevel.as_str() {
        "error" => {
            log::set_max_level(log::LevelFilter::Error);
        }
        "warn" => {
            log::set_max_level(log::LevelFilter::Warn);
        }
        "info" => {
            log::set_max_level(log::LevelFilter::Info);
        }
        "debug" => {
            log::set_max_level(log::LevelFilter::Debug);
        }
        "trace" => {
            log::set_max_level(log::LevelFilter::Trace);
        }
        _ => {
            log::set_max_level(log::LevelFilter::Info);
        }
    }

    let openai_key = env::var("OPENAI_API_KEY")
        .ok()
        .unwrap_or_else(|| "NO_API_KEY".to_string());

    if args.use_openai && openai_key == "NO_API_KEY" {
        error!("OpenAI API key is not set. Please set the OPENAI_API_KEY environment variable.");
        std::process::exit(1);
    }

    let system_prompt = args.system_prompt;
    let query = args.query;

    let system_message = Message {
        role: "system".to_string(),
        content: system_prompt.to_string(),
    };

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
    let ai_os_stats = args.ai_os_stats;
    let ai_network_stats = args.ai_network_stats;

    if args.use_openai {
        // set the llm_host to the openai api
        llm_host = "https://api.openai.com".to_string();
    }

    // start time
    let start_time = current_unix_timestamp_ms().unwrap_or(0);

    // Perform TR 101 290 checks
    let mut tr101290_errors = Tr101290Errors::new();
    // calculate read size based on batch size and packet size
    let read_size: i32 =
        (args.packet_size as i32 * args.pcap_batch_size as i32) + args.payload_offset as i32; // pcap read size
    let mut is_mpegts = true; // Default to true, update based on actual packet type

    let (ptx, mut prx) = mpsc::channel::<Arc<Vec<u8>>>(args.pcap_channel_size);
    let (batch_tx, mut batch_rx) = mpsc::channel::<String>(args.pcap_channel_size); // Channel for passing processed packets to main logic
    let mut network_capture_config = NetworkCapture {
        running: Arc::new(AtomicBool::new(true)),
        dpdk: false,
        use_wireless: args.use_wireless,
        promiscuous: args.promiscuous,
        immediate_mode: args.immediate_mode,
        source_protocol: Arc::new(args.source_protocol.to_string()),
        source_device: Arc::new(args.source_device.to_string()),
        source_ip: Arc::new(args.source_ip.to_string()),
        source_port: args.source_port,
        read_time_out: 60_000,
        read_size,
        buffer_size: args.buffer_size,
        pcap_stats: args.pcap_stats,
        debug_on: args.hexdump,
        capture_task: None,
    };

    // Initialize messages with system_message outside the loop
    let mut messages = vec![system_message];

    // Initialize the network capture if ai_network_stats is true
    if ai_network_stats {
        println!("Starting network capture");
        network_capture(&mut network_capture_config, ptx);
        println!("Network capture started");
    }

    let processing_handle = tokio::spawn(async move {
        let mut decode_batch = Vec::new();
        let mut video_pid: Option<u16> = Some(0xFFFF);
        let mut video_codec: Option<Codec> = Some(Codec::NONE);
        let mut current_video_frame = Vec::<StreamData>::new();
        let mut pmt_info: PmtInfo = PmtInfo {
            pid: 0xFFFF,
            packet: Vec::new(),
        };

        let mut packet_last_sent_ts = Instant::now();
        let mut count = 0;
        loop {
            if ai_network_stats {
                debug!("Capturing network packets...");
                while let Some(packet) = prx.recv().await {
                    count += 1;
                    debug!(
                        "#{} --- Received packet with size: {} bytes",
                        count,
                        packet.len()
                    );

                    // Check if chunk is MPEG-TS or SMPTE 2110
                    let chunk_type = is_mpegts_or_smpte2110(&packet[args.payload_offset..]);
                    if chunk_type != 1 {
                        if chunk_type == 0 {
                            hexdump(&packet, 0, packet.len());
                            error!("Not MPEG-TS or SMPTE 2110");
                        }
                        is_mpegts = false;
                    }

                    // Process the packet here
                    let chunks = if is_mpegts {
                        process_mpegts_packet(
                            args.payload_offset,
                            packet,
                            args.packet_size,
                            start_time,
                        )
                    } else {
                        process_smpte2110_packet(
                            args.payload_offset,
                            packet,
                            args.packet_size,
                            start_time,
                            false,
                        )
                    };

                    // Process each chunk
                    for mut stream_data in chunks {
                        // check for null packets of the pid 8191 0x1FFF and skip them
                        if stream_data.pid >= 0x1FFF {
                            debug!("Skipping null packet");
                            continue;
                        }

                        if args.hexdump {
                            hexdump(
                                &stream_data.packet,
                                stream_data.packet_start,
                                stream_data.packet_len,
                            );
                        }

                        // Extract the necessary slice for PID extraction and parsing
                        let packet_chunk = &stream_data.packet[stream_data.packet_start
                            ..stream_data.packet_start + stream_data.packet_len];

                        if is_mpegts {
                            let pid = stream_data.pid;
                            // Handle PAT and PMT packets
                            match pid {
                                PAT_PID => {
                                    debug!("ProcessPacket: PAT packet detected with PID {}", pid);
                                    pmt_info = parse_and_store_pat(&packet_chunk);
                                    // Print TR 101 290 errors
                                    if args.show_tr101290 {
                                        info!("STATUS::TR101290:ERRORS: {}", tr101290_errors);
                                    }
                                }
                                _ => {
                                    // Check if this is a PMT packet
                                    if pid == pmt_info.pid {
                                        debug!(
                                            "ProcessPacket: PMT packet detected with PID {}",
                                            pid
                                        );
                                        // Update PID_MAP with new stream types
                                        update_pid_map(&packet_chunk, &pmt_info.packet);
                                        // Identify the video PID (if not already identified)
                                        if let Some((new_pid, new_codec)) =
                                            identify_video_pid(&packet_chunk)
                                        {
                                            if video_pid.map_or(true, |vp| vp != new_pid) {
                                                video_pid = Some(new_pid);
                                                info!(
                                                    "STATUS::VIDEO_PID:CHANGE: to {}/{} from {}/{}",
                                                    new_pid,
                                                    new_codec.clone(),
                                                    video_pid.unwrap(),
                                                    video_codec.unwrap()
                                                );
                                                video_codec = Some(new_codec.clone());
                                                // Reset video frame as the video stream has changed
                                                current_video_frame.clear();
                                            } else if video_codec != Some(new_codec.clone()) {
                                                info!(
                                                    "STATUS::VIDEO_CODEC:CHANGE: to {} from {}",
                                                    new_codec,
                                                    video_codec.unwrap()
                                                );
                                                video_codec = Some(new_codec);
                                                // Reset video frame as the codec has changed
                                                current_video_frame.clear();
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Check for TR 101 290 errors
                        process_packet(
                            &mut stream_data,
                            &mut tr101290_errors,
                            is_mpegts,
                            pmt_info.pid,
                        );
                        count += 1;

                        decode_batch.push(stream_data);
                    }

                    // check if it is 60 seconds since the last packet was sent
                    let last_packet_sent = packet_last_sent_ts.elapsed().as_secs();

                    // If the batch is full, process it
                    if args.poll_interval == 0
                        || (last_packet_sent > (args.poll_interval / 1000)
                            && decode_batch.len() > args.ai_network_packet_count)
                    {
                        let mut network_packet_dump: String = String::new();
                        packet_last_sent_ts = Instant::now();

                        network_packet_dump.push_str("\n");
                        // fill network_packet_dump with the json of each stream_data plus hexdump of the packet payload
                        for stream_data in &decode_batch {
                            if args.ai_network_packets {
                                let stream_data_json = serde_json::to_string(&stream_data).unwrap();
                                network_packet_dump.push_str(&stream_data_json);
                                network_packet_dump.push_str("\n");
                            }

                            // hex of the packet_chunk with ascii representation after | for each line
                            if args.ai_network_hexdump {
                                // Extract the necessary slice for PID extraction and parsing
                                let packet_chunk = &stream_data.packet[stream_data.packet_start
                                    ..stream_data.packet_start + stream_data.packet_len];

                                network_packet_dump.push_str(&hexdump_ascii(
                                    &packet_chunk,
                                    0,
                                    (stream_data.packet_start + stream_data.packet_len)
                                        - stream_data.packet_start,
                                ));
                                network_packet_dump.push_str("\n");
                            }
                        }
                        // get PID_MAP and each stream data in json format and send it to the main thread
                        // get pretty date and time
                        let pretty_date_time = format!(
                            "#{}: {}",
                            count,
                            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
                        );
                        let pid_map = format!("{}: {}", pretty_date_time, get_pid_map());
                        network_packet_dump.push_str(&pid_map);

                        // Send the network packet dump to the Main thread
                        if let Err(e) = batch_tx.send(network_packet_dump.clone()).await {
                            eprintln!("Failed to send decode batch: {}", e);
                        }

                        // empty decode_batch
                        decode_batch.clear();
                    }
                }
            } else {
                // sleep for a while to avoid busy loop
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    });

    let poll_interval = args.poll_interval;
    let poll_interval_duration = Duration::from_millis(poll_interval);
    let mut poll_start_time = Instant::now();
    let mut dot_last_sent_ts = Instant::now();
    info!(
        "Starting up RsLLM with poll intervale of {} seconds...",
        poll_interval_duration.as_secs()
    );
    let mut count = 0;
    loop {
        count += 1;

        // OS and Network stats message
        let system_stats_json = if ai_os_stats {
            get_stats_as_json(StatsType::System).await
        } else {
            // Default input message
            json!({})
        };

        // Add the system stats to the messages
        if !ai_os_stats && !ai_network_stats {
            let user_message = Message {
                role: "user".to_string(),
                content: query.to_string(),
            };
            messages.push(user_message.clone());
        } else if ai_network_stats {
            // create nework packet dump message from collected stream_data in decode_batch
            // Try to receive new packet batches if available
            let mut msg_count = 0;
            while let Ok(decode_batch) = batch_rx.try_recv() {
                if !args.no_progress && dot_last_sent_ts.elapsed().as_secs() >= 1 {
                    dot_last_sent_ts = Instant::now();
                    print!("*");
                    // Flush stdout to ensure the progress dots are printed
                    io::stdout().flush().unwrap();
                }
                msg_count += 1;
                //debug!("Received network packet dump message: {}", decode_batch);
                // Handle the received decode_batch here...
                // get current pretty date and time
                let pretty_date_time = format!(
                    "#{}: {} -",
                    count,
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
                );
                let network_stats_message = Message {
                    role: "user".to_string(),
                    content: format!(
                        "{} System Stats: {}\nPackets: {}\nInstructions: {}\n",
                        pretty_date_time,
                        system_stats_json.to_string(),
                        decode_batch,
                        query
                    ),
                };
                messages.push(network_stats_message.clone());
                if msg_count >= 1 {
                    break;
                }
            }
        } else if ai_os_stats {
            let pretty_date_time = format!(
                "#{}: {} - ",
                count,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f")
            );
            let system_stats_message = Message {
                role: "user".to_string(),
                content: format!(
                    "{} System Stats: {}\nInstructions: {}",
                    pretty_date_time,
                    system_stats_json.to_string(),
                    query
                ),
            };
            messages.push(system_stats_message.clone());
        }

        // Debugging LLM history
        if args.debug_llm_history {
            // print out the messages to the console
            info!("---");
            info!("Messages:");
            for message in &messages {
                info!("{}: {}", message.role, message.content);
            }
            info!("---");
        }

        // measure size of messages in bytes and print it out
        let messages_size = bincode::serialize(&messages).unwrap().len();
        info!("Initial Messages size: {}", messages_size);

        let llm_history_size_bytes: usize = args.llm_history_size; // Your defined max size in bytes

        // Separate system messages to preserve them
        let (system_messages, mut non_system_messages): (Vec<_>, Vec<_>) =
            messages.into_iter().partition(|m| m.role == "system");

        let total_non_system_size: usize =
            non_system_messages.iter().map(|m| m.content.len()).sum();

        // If non-system messages alone exceed the limit, we need to trim
        if llm_history_size_bytes > 0 && total_non_system_size > llm_history_size_bytes {
            let mut excess_size = total_non_system_size - llm_history_size_bytes;

            // Reverse iterate to trim from the end
            for message in non_system_messages.iter_mut().rev() {
                let message_size = message.content.len();
                if excess_size == 0 {
                    break;
                }

                if message_size <= excess_size {
                    // Remove the whole message content if it's smaller than or equal to the excess
                    excess_size -= message_size;
                    message.content.clear();
                } else {
                    // Truncate the message content to fit within the limit
                    let new_size = message_size - excess_size;
                    message.content = message.content.chars().take(new_size).collect();
                    break; // After truncation, we should be within the limit
                }
            }
        }

        // Reassemble messages, ensuring system messages are preserved at their original position
        messages = system_messages
            .into_iter()
            .chain(non_system_messages.into_iter())
            .collect();

        let adjusted_messages_size = messages.iter().map(|m| m.content.len()).sum::<usize>();
        if messages_size != adjusted_messages_size {
            debug!(
                "Messages size (bytes of content) adjusted from {} to {} for {} messages.",
                messages_size,
                adjusted_messages_size,
                messages.len()
            );
        } else {
            debug!(
                "Messages size {} for {} messages.",
                messages_size,
                messages.len()
            );
        }

        // Debug print to show the content sizes and roles
        if args.debug_llm_history {
            debug!("Message History:");
            for (i, message) in messages.iter().enumerate() {
                debug!(
                    "Message {} - Role: {}, Size: {}",
                    i + 1,
                    message.role,
                    message.content.len()
                );
            }
        }

        // Stream API Completion
        let stream = !args.no_stream;
        let open_ai_request = OpenAIRequest {
            model: &model,
            max_tokens: &max_tokens, // add this field to the request struct
            messages: messages.clone(),
            temperature: &temperature, // add this field to the request struct
            top_p: &top_p,             // add this field to the request struct
            presence_penalty: &presence_penalty, // add this field to the request struct
            frequency_penalty: &frequency_penalty, // add this field to the request struct
            stream: &stream,
        };

        // Directly await the future; no need for an explicit runtime block
        let answers = stream_completion(
            open_ai_request,
            &openai_key,
            &llm_host,
            &llm_path,
            debug_inline,
            args.show_output_errors,
            args.break_line_length,
        )
        .await
        .unwrap_or_else(|_| Vec::new());

        // for each answer in the response
        for answer in answers {
            let assistant_message = Message {
                role: "assistant".to_string(),
                content: answer.content,
            };

            // push the message to the open_ai_request
            messages.push(assistant_message.clone());
        }

        // break the loop if we are not running as a daemon
        if !args.daemon {
            break;
        }

        // Calculate elapsed time since last start
        let elapsed = poll_start_time.elapsed();

        // Sleep only if the elapsed time is less than the poll interval
        if elapsed < poll_interval_duration {
            // Sleep only if the elapsed time is less than the poll interval
            info!(
                "Sleeping for {} ms...",
                poll_interval_duration.as_millis() - elapsed.as_millis()
            );
            tokio::time::sleep(poll_interval_duration - elapsed).await;
            info!("Running after sleeping...");
        }

        // Update start time for the next iteration
        poll_start_time = Instant::now();
    }

    // Close the network capture if ai_network_stats is true
    if ai_network_stats {
        network_capture_config
            .running
            .store(false, Ordering::SeqCst);
    }

    // Await the completion of background tasks
    let _ = processing_handle.await;
}
