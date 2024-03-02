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

use clap::Parser;
use log::{debug, error, info};
use rsllm::candle_gemma::gemma;
use rsllm::candle_mistral::mistral;
#[cfg(feature = "ndi")]
use rsllm::ndi::send_audio_samples_over_ndi;
#[cfg(feature = "ndi")]
use rsllm::ndi::send_images_over_ndi;
use rsllm::network_capture::{network_capture, NetworkCapture};
use rsllm::openai_api::{format_messages_for_llama2, stream_completion, Message, OpenAIRequest};
use rsllm::openai_tts::tts as oai_tts;
use rsllm::openai_tts::Request as OAITTSRequest;
use rsllm::openai_tts::Voice as OAITTSVoice;
use rsllm::stable_diffusion::{sd, SDConfig};
use rsllm::stream_data::{
    get_pid_map, identify_video_pid, is_mpegts_or_smpte2110, parse_and_store_pat, process_packet,
    update_pid_map, Codec, PmtInfo, StreamData, Tr101290Errors, PAT_PID,
};
use rsllm::stream_data::{process_mpegts_packet, process_smpte2110_packet};
use rsllm::{current_unix_timestamp_ms, hexdump, hexdump_ascii};
use rsllm::{get_stats_as_json, StatsType};
use serde_json::{self, json};
use std::env;
use std::io::Write;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tokio::sync::mpsc::{self};
use tokio::sync::Semaphore;
use tokio::time::Duration;
use uuid::Uuid;

/// RScap Probe Configuration
#[derive(Parser, Debug)]
#[clap(
    author = "Chris Kennedy",
    version = "2.2",
    about = "Rust LLM Stream Analyzer and Content Generator"
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

    /// Model ID - for gemma 2b or 7b, mistral has various options too
    #[clap(
        long,
        env = "MODEL_ID",
        default_value = "auto",
        help = "Model ID - model path on huggingface or 7b / 2b for gemma"
    )]
    model_id: String,

    /// Quantized bool
    #[clap(
        long,
        env = "QUANTIZED",
        default_value = "false",
        help = "Quantized, it will use a quantized LLM to generate output faster on CPUs or GPUs."
    )]
    quantized: bool,

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

    /// OAI_TTS as text to speech from openai
    #[clap(
        long,
        env = "OAI_TTS",
        default_value = "false",
        help = "OAI_TTS as text to speech from openai, default is false."
    )]
    oai_tts: bool,

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

    /// POLL Interval in ms
    #[clap(
        long,
        env = "POLL_INTERVAL",
        default_value_t = 10_000,
        help = "POLL Interval in ms."
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

    /// Break Line Length - line length for breaking lines from LLM messages, default is 80
    #[clap(
        long,
        env = "BREAK_LINE_LENGTH",
        default_value_t = 80,
        help = "Break Line Length - line length for breaking lines from LLM messages, default is 80."
    )]
    break_line_length: usize,

    /// SD Image - create an SD image from the LLM messages
    #[clap(
        long,
        env = "SD_IMAGE",
        default_value_t = false,
        help = "SD Image - create an SD image from the LLM messages, default is false."
    )]
    sd_image: bool,

    /// SD Max Length for SD Image
    #[clap(
        long,
        env = "SD_MAX_LENGTH",
        default_value_t = 120,
        help = "SD Max Length for SD Image, default is 200."
    )]
    sd_max_length: usize,

    /// Save Images - save images from the LLM messages
    #[clap(
        long,
        env = "SAVE_IMAGES",
        default_value_t = false,
        help = "Save Images - save images from the LLM messages, default is false."
    )]
    save_images: bool,

    /// NDI output
    #[clap(
        long,
        env = "NDI_IMAGES",
        default_value_t = false,
        help = "NDI Images output, default is false. (use --features ndi to enable NDI)"
    )]
    ndi_images: bool,

    /// NDI Audio
    #[clap(
        long,
        env = "NDI_AUDIO",
        default_value_t = false,
        help = "NDI Audio output, default is false. (use --features ndi to enable NDI)"
    )]
    ndi_audio: bool,

    /// Max Iterations
    #[clap(
        long,
        env = "MAX_ITERATIONS",
        default_value_t = 1,
        help = "Max Iterations, default is 1."
    )]
    max_iterations: i32,

    /// Use Candle for LLM
    #[clap(
        long,
        env = "USE_CANDLE",
        default_value_t = false,
        help = "Use Candle for LLM, default is false."
    )]
    use_candle: bool,

    /// which llm to use from candle, string default is mistral
    #[clap(
        long,
        env = "CANDLE_LLM",
        default_value = "mistral",
        help = "which llm to use from candle, string default is mistral."
    )]
    candle_llm: String,
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

    let system_prompt = args.system_prompt;

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
        network_capture(&mut network_capture_config, ptx);
    }

    let running_processor = Arc::new(AtomicBool::new(true));
    let running_processor_clone = running_processor.clone();

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
        while running_processor_clone.load(Ordering::SeqCst) {
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
    if args.daemon {
        println!(
            "Starting up RsLLM with poll interval of {} seconds...",
            poll_interval_duration.as_secs()
        );
    } else {
        println!("Running RsLLM #{} iterations...", args.max_iterations);
    }
    let mut count = 0;
    loop {
        let openai_key = env::var("OPENAI_API_KEY")
            .ok()
            .unwrap_or_else(|| "NO_API_KEY".to_string());

        if (args.use_openai || args.oai_tts) && openai_key == "NO_API_KEY" {
            error!(
                "OpenAI API key is not set. Please set the OPENAI_API_KEY environment variable."
            );
            std::process::exit(1);
        }

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
            let query_clone = args.query.clone();

            let user_message = Message {
                role: "user".to_string(),
                content: query_clone.to_string(),
            };
            messages.push(user_message.clone());
        } else if ai_network_stats {
            // create nework packet dump message from collected stream_data in decode_batch
            // Try to receive new packet batches if available
            let mut msg_count = 0;
            while let Ok(decode_batch) = batch_rx.try_recv() {
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
                        args.query
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
                    args.query
                ),
            };
            messages.push(system_stats_message.clone());
        }

        // Debugging LLM history
        if args.debug_llm_history {
            // print out the messages to the console
            println!("==============================");
            println!("Messages:");
            println!("==============================");
            for message in &messages {
                println!("{}: {}\n---\n", message.role, message.content);
            }
            println!("============= NEW RESPONSE ============");
        } else {
            println!("============= NEW RESPONSE ============");
        }

        // measure size of messages in bytes and print it out
        let messages_size = bincode::serialize(&messages).unwrap().len();
        debug!("Initial Messages size: {}", messages_size);

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

        // Setup mpsc channels for internal communication within the mistral function
        let (external_sender, mut external_receiver) = tokio::sync::mpsc::channel::<String>(32768);

        let model_id = args.model_id.clone();

        if args.use_candle {
            // Capture the start time for performance metrics
            let start = Instant::now();

            let chat_format = if args.candle_llm == "mistral" {
                "llama2".to_string()
            } else if args.candle_llm == "gemma" {
                if args.model_id == "7b-it" {
                    "google".to_string()
                } else if args.model_id == "2b-it" {
                    "google".to_string()
                } else {
                    "".to_string()
                }
            } else {
                "llama2".to_string()
            };

            let prompt = format_messages_for_llama2(messages.clone(), chat_format);

            debug!("\nPrompt: {}", prompt);

            // Spawn a thread to run the mistral function, to keep the UI responsive
            if args.candle_llm != "mistral" && args.candle_llm != "gemma" {
                // exit if the LLM is not supported
                error!("The specified LLM is not supported. Exiting...");
                std::process::exit(1);
            }

            let prompt_clone = prompt.clone();
            let llm_thread = if args.candle_llm == "mistral" {
                tokio::spawn(async move {
                    if let Err(e) = mistral(
                        prompt_clone,
                        max_tokens as usize,
                        temperature as f64,
                        args.quantized,
                        Some(model_id),
                        external_sender,
                    ) {
                        eprintln!("Error running mistral: {}", e);
                    }
                })
            } else {
                tokio::spawn(async move {
                    if let Err(e) = gemma(
                        prompt_clone,
                        max_tokens as usize,
                        temperature as f64,
                        args.quantized,
                        Some(model_id),
                        external_sender,
                    ) {
                        eprintln!("Error running gemma: {}", e);
                    }
                })
            };

            // Count tokens and collect output
            let mut token_count = 0;
            let mut answers = Vec::new();
            let mut paragraphs: Vec<String> = Vec::new();
            let mut current_paragraph: Vec<String> = Vec::new();
            let mut paragraph_count = 0;
            let min_paragraph_len = 40; // Minimum length of a paragraph to generate an image
            let mut image_spawn_handles = Vec::new();

            // Stable Diffusion number of tasks max
            // Before starting  loop, initialize the semaphore with a specific number of permits
            let max_concurrent_tasks = 3; // Set this to the desired number of concurrent tasks
            let semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));

            // create uuid unique identifier for the output images
            let output_id = Uuid::new_v4().simple().to_string(); // Generates a UUID and converts it to a simple, hyphen-free string

            while let Some(received) = external_receiver.recv().await {
                token_count += 1;

                // Store the received token
                answers.push(received.clone());

                // If a newline is at the end of the token, process the accumulated paragraph for image generation
                if received.contains('\n')
                    || (current_paragraph.join("").len() > args.sd_max_length
                        && (received.contains('.')
                            || received.contains('?')
                            || received.contains(' ')
                            || received.contains('!')))
                {
                    // Join the current paragraph tokens into a single String without adding extra spaces
                    if !current_paragraph.is_empty()
                        && current_paragraph.join("").len() > min_paragraph_len
                    {
                        // check if token has the new line character, split it at the new line into two parts, then put the first part onto
                        // the current paragraph and the second part into the answers and current_paragraph later after we store the current paragraph
                        // Safely handle split at the newline character
                        let mut split_pos = received.len();
                        for delimiter in ['\n', '.', '?', '!'] {
                            if let Some(pos) = received.find(delimiter) {
                                // Adjust position to keep the delimiter with the first part, except for '\n'
                                let end_pos = if delimiter == '\n' { pos } else { pos + 1 };
                                split_pos = split_pos.min(end_pos);
                                break; // Break after finding the first delimiter
                            }
                        }
                        // Handle ' ' delimiter separately
                        if split_pos == received.len() {
                            if let Some(pos) = received.find(' ') {
                                // Adjust position to keep the delimiter with the first part, except for '\n'
                                let end_pos = pos + 1;
                                split_pos = split_pos.min(end_pos);
                            }
                        }

                        // Split 'received' at the determined position, handling '\n' specifically
                        let (mut first, mut second) = received.split_at(split_pos);

                        // If splitting on '\n', adjust 'first' and 'second' to not include '\n' in 'first'
                        if first.ends_with('\n') {
                            first = &first[..first.len() - 1];
                        } else if second.starts_with('\n') {
                            second = &second[1..];
                        }

                        // Store the first part of the split token into the current paragraph
                        current_paragraph.push(first.to_string());

                        let paragraph_text = current_paragraph.join(""); // Join without spaces as indicated
                        paragraphs.push(paragraph_text.clone());

                        // Clear current paragraph for the next one
                        current_paragraph.clear(); // Clear current paragraph for the next one

                        // Store the second part of the split token into the answers and current_paragraph
                        current_paragraph.push(second.to_string());

                        // Token output to stdout in real-time
                        print!("{}", first);
                        std::io::stdout().flush().unwrap();

                        // Check if image generation is enabled and proceed
                        if args.sd_image {
                            // Clone necessary data for use in the async block
                            let paragraph_clone = paragraphs[paragraph_count].clone();

                            std::io::stdout().flush().unwrap();
                            println!(
                                "\n===\nGenerating image {} #{} for {} characters...\n===\n",
                                output_id,
                                paragraph_count,
                                paragraph_clone.len()
                            );

                            let output_id_clone = output_id.clone();

                            let sem_clone = semaphore.clone();
                            let handle = tokio::spawn(async move {
                                let _permit = sem_clone.acquire().await.expect(
                                    "Stable Diffusion Thread: Failed to acquire semaphore permit",
                                );

                                let mut sd_config = SDConfig::new();
                                sd_config.prompt = paragraph_clone;
                                sd_config.height = Some(512);
                                sd_config.width = Some(512);

                                let prompt_clone = sd_config.prompt.clone();

                                debug!("Generating images with prompt: {}", sd_config.prompt);

                                match sd(sd_config).await {
                                    // Ensure `sd` function is async and await its result
                                    Ok(images) => {
                                        // Send images over NDI
                                        #[cfg(feature = "ndi")]
                                        if args.ndi_images {
                                            #[cfg(feature = "ndi")]
                                            if args.ndi_images {
                                                debug!("Sending images over NDI");
                                            }
                                            #[cfg(feature = "ndi")]
                                            send_images_over_ndi(images.clone(), &prompt_clone)
                                                .unwrap();
                                        }

                                        if args.oai_tts {
                                            let input = prompt_clone.clone();
                                            let model = String::from("tts-1");
                                            let voice = OAITTSVoice::Nova;
                                            let oai_request =
                                                OAITTSRequest::new(input, model, voice);

                                            let openai_key = std::env::var("OPENAI_API_KEY")
                                                .expect(
                                                "Stable Diffusion Thread: OPENAI_API_KEY not found",
                                            );

                                            let tts_thread: tokio::task::JoinHandle<
                                                Result<(), Box<dyn std::error::Error + Send>>,
                                            > = tokio::spawn(async move {
                                                let bytes_result =
                                                    oai_tts(oai_request, &openai_key).await;

                                                match bytes_result {
                                                    Ok(bytes) => {
                                                        if args.ndi_audio {
                                                            // Convert MP3 bytes to f32 samples
                                                            #[cfg(feature = "ndi")]
                                                            let samples_result =
                                                                rsllm::ndi::mp3_to_f32(
                                                                    bytes.to_vec(),
                                                                );

                                                            #[cfg(feature = "ndi")]
                                                            match samples_result {
                                                                Ok(samples_f32) => {
                                                                    {
                                                                        // samples_f32 is a Vec<f32> containing your audio samples
                                                                        // send_audio_samples_over_ndi expects a Vec<f32>, 32000 as sample rate, and 2 as channel count.
                                                                        send_audio_samples_over_ndi(
                                                                                       samples_f32,
                                                                                       32000, // Sample rate
                                                                                       2,     // Channel count
                                                                                   )
                                                                                   .expect("Failed to send audio samples over NDI");
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    eprintln!("Failed to convert MP3 bytes to f32 samples: {:?}", e);
                                                                }
                                                            }
                                                        } else {
                                                            // Play audio
                                                            let (_stream, stream_handle) =
                                                                rodio::OutputStream::try_default()
                                                                    .unwrap();
                                                            let sink = rodio::Sink::try_new(
                                                                &stream_handle,
                                                            )
                                                            .unwrap();
                                                            let cursor =
                                                                std::io::Cursor::new(bytes);
                                                            let decoder_result =
                                                                rodio::Decoder::new_mp3(cursor);
                                                            match decoder_result {
                                                                Ok(source) => {
                                                                    sink.append(source);
                                                                    sink.sleep_until_end();
                                                                }
                                                                Err(e) => {
                                                                    // Log or handle the error as needed
                                                                    eprintln!(
                                                                        "Error decoding MP3: {}",
                                                                        e
                                                                    );
                                                                    return Err(Box::new(e)
                                                                        as Box<
                                                                            dyn std::error::Error
                                                                                + Send,
                                                                        >);
                                                                }
                                                            }
                                                        }
                                                        Ok(())
                                                    }
                                                    Err(e) => {
                                                        // Log or handle the error as needed
                                                        eprintln!("Error in TTS request: {}", e);
                                                        Err(Box::new(e)
                                                            as Box<dyn std::error::Error + Send>)
                                                    }
                                                }
                                            });

                                            match tts_thread.await {
                                                Ok(result) => {
                                                    result
                                                        .expect("TTS thread encountered an error");
                                                }
                                                Err(e) => {
                                                    // Handle the join error (if the spawned task panicked)
                                                    eprintln!("Error joining TTS thread: {:?}", e);
                                                }
                                            }
                                        }

                                        // Save images to disk
                                        if args.save_images {
                                            for (index, image_bytes) in images.iter().enumerate() {
                                                let image_file = format!(
                                                    "images/{}_{}_{}.png",
                                                    output_id_clone, paragraph_count, index
                                                );
                                                debug!(
                                                    "Image {} {}/{} saving to {}",
                                                    output_id_clone,
                                                    paragraph_count,
                                                    index,
                                                    image_file
                                                );
                                                image_bytes
                                                    .save(image_file)
                                                    .map_err(candle_core::Error::wrap)
                                                    .unwrap(); // And this as well
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Error generating images for {}: {:?}",
                                            output_id_clone, e
                                        );
                                    }
                                }
                            });
                            image_spawn_handles.push(handle);
                        }

                        // Token output to stdout in real-time
                        print!("{}", second);
                        std::io::stdout().flush().unwrap();

                        paragraph_count += 1; // Increment paragraph count for the next paragraph
                    } else {
                        // Token output to stdout in real-time
                        print!("{}", received);
                        std::io::stdout().flush().unwrap();

                        // store the token in the current paragraph
                        current_paragraph.push(received.clone());
                    }
                } else {
                    // Token output to stdout in real-time
                    print!("{}", received);
                    std::io::stdout().flush().unwrap();

                    // store the token in the current paragraph
                    current_paragraph.push(received.clone());
                }
            }

            // Join the last paragraph tokens into a single String without adding extra spaces
            if current_paragraph.len() > 0 {
                // TODO: do anything needed with the last paragraph bits like TTS sending
                let paragraph_text = current_paragraph.join(""); // Join without spaces as indicated
                let paragraph_text_clone = paragraph_text.clone();

                let output_id_clone = output_id.clone();

                // end of the last paragraph image generation
                let handle = tokio::spawn(async move {
                    let mut sd_config = SDConfig::new();
                    sd_config.prompt = paragraph_text_clone;
                    sd_config.height = Some(512);
                    sd_config.width = Some(512);

                    let prompt_clone = sd_config.prompt.clone();

                    debug!("Generating images with prompt: {}", sd_config.prompt);

                    match sd(sd_config).await {
                        // Ensure `sd` function is async and await its result
                        Ok(images) => {
                            // Send images over NDI
                            if args.ndi_images {
                                debug!("Sending images over NDI");
                            }
                            #[cfg(feature = "ndi")]
                            if args.ndi_images {
                                #[cfg(feature = "ndi")]
                                send_images_over_ndi(images.clone(), &prompt_clone).unwrap();
                                // This is now allowed
                            }

                            if args.oai_tts {
                                let input = prompt_clone.clone();
                                let model = String::from("tts-1");
                                let voice = OAITTSVoice::Nova;
                                let oai_request = OAITTSRequest::new(input, model, voice);

                                let openai_key = std::env::var("OPENAI_API_KEY")
                                    .expect("Stable Diffusion Thread: OPENAI_API_KEY not found");

                                let tts_thread: tokio::task::JoinHandle<
                                    Result<(), Box<dyn std::error::Error + Send>>,
                                > = tokio::spawn(async move {
                                    let bytes_result = oai_tts(oai_request, &openai_key).await;

                                    match bytes_result {
                                        Ok(bytes) => {
                                            if args.ndi_audio {
                                                // Convert MP3 bytes to f32 samples
                                                #[cfg(feature = "ndi")]
                                                let samples_result =
                                                    rsllm::ndi::mp3_to_f32(bytes.to_vec());

                                                #[cfg(feature = "ndi")]
                                                match samples_result {
                                                    Ok(samples_f32) => {
                                                        {
                                                            // samples_f32 is a Vec<f32> containing your audio samples
                                                            // send_audio_samples_over_ndi expects a Vec<f32>, 32000 as sample rate, and 2 as channel count.
                                                            send_audio_samples_over_ndi(
                                                                           samples_f32,
                                                                           32000, // Sample rate
                                                                           2,     // Channel count
                                                                       )
                                                                       .expect("Failed to send audio samples over NDI");
                                                        }
                                                    }
                                                    Err(e) => {
                                                        eprintln!("Failed to convert MP3 bytes to f32 samples: {:?}", e);
                                                    }
                                                }
                                            } else {
                                                // Play audio
                                                let (_stream, stream_handle) =
                                                    rodio::OutputStream::try_default().unwrap();
                                                let sink =
                                                    rodio::Sink::try_new(&stream_handle).unwrap();
                                                let cursor = std::io::Cursor::new(bytes);
                                                let decoder_result =
                                                    rodio::Decoder::new_mp3(cursor);
                                                match decoder_result {
                                                    Ok(source) => {
                                                        sink.append(source);
                                                        sink.sleep_until_end();
                                                    }
                                                    Err(e) => {
                                                        // Log or handle the error as needed
                                                        eprintln!("Error decoding MP3: {}", e);
                                                        return Err(Box::new(e)
                                                            as Box<dyn std::error::Error + Send>);
                                                    }
                                                }
                                            }
                                            Ok(())
                                        }
                                        Err(e) => {
                                            // Log or handle the error as needed
                                            eprintln!("Error in TTS request: {}", e);
                                            Err(Box::new(e) as Box<dyn std::error::Error + Send>)
                                        }
                                    }
                                });

                                match tts_thread.await {
                                    Ok(result) => {
                                        result.expect("TTS thread encountered an error");
                                    }
                                    Err(e) => {
                                        // Handle the join error (if the spawned task panicked)
                                        eprintln!("Error joining TTS thread: {:?}", e);
                                    }
                                }
                            }

                            // Save images to disk
                            if args.save_images {
                                for (index, image_bytes) in images.iter().enumerate() {
                                    let image_file = format!(
                                        "images/{}_{}_{}.png",
                                        output_id_clone, paragraph_count, index
                                    );
                                    debug!(
                                        "\nImage {} {}/{} saving to {}",
                                        output_id_clone, paragraph_count, index, image_file
                                    );
                                    image_bytes
                                        .save(image_file)
                                        .map_err(candle_core::Error::wrap)
                                        .unwrap(); // And this as well
                                }
                            }
                        }
                        Err(e) => {
                            std::io::stdout().flush().unwrap();
                            eprintln!("Error generating images for {}: {:?}", output_id_clone, e);
                        }
                    }
                });
                image_spawn_handles.push(handle);
                paragraph_count += 1; // Increment paragraph count for the next paragraph
            }

            // Wait for the LLM thread to finish
            llm_thread.await.unwrap();

            // Calculate elapsed time and tokens per second
            let elapsed = start.elapsed().as_secs_f64();
            let tokens_per_second = token_count as f64 / elapsed;

            let answers_str = answers.join("").to_string();

            println!(
                "\n================================\n#{} Generated {}/{}/{} paragraphs/tokens/chars in {:.2?} seconds ({:.2} tokens/second)\n================================\n",
                output_id, paragraph_count, token_count, answers_str.len(), elapsed, tokens_per_second
            );

            // add answers to the messages as an assistant role message with the content
            messages.push(Message {
                role: "assistant".to_string(),
                content: answers_str.clone(),
            });

            // wait for the image generation to finish
            for handle in image_spawn_handles {
                handle.await.unwrap();
            }
        } else {
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
                &openai_key.clone(),
                &llm_host,
                &llm_path,
                debug_inline,
                args.show_output_errors,
                args.break_line_length,
                args.sd_image,
                args.ndi_images,
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
        }

        // break the loop if we are not running as a daemon or hit max iterations
        if (!args.daemon && args.max_iterations <= 1)
            || (args.max_iterations > 1 && args.max_iterations == count)
        {
            // stop the running threads
            if ai_network_stats {
                network_capture_config
                    .running
                    .store(false, Ordering::SeqCst);
            }

            // stop the running threads
            running_processor.store(false, Ordering::SeqCst);

            // Await the completion of background tasks
            let _ = processing_handle.await;

            break;
        }

        // Calculate elapsed time since last start
        let elapsed = poll_start_time.elapsed();

        // Sleep only if the elapsed time is less than the poll interval
        if elapsed < poll_interval_duration {
            // Sleep only if the elapsed time is less than the poll interval
            println!(
                "Sleeping for {} ms...",
                poll_interval_duration.as_millis() - elapsed.as_millis()
            );
            tokio::time::sleep(poll_interval_duration - elapsed).await;
            println!("Running after sleeping...");
        }

        // Update start time for the next iteration
        poll_start_time = Instant::now();
    }
}
