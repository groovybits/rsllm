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
use rsllm::args::Args;
use rsllm::candle_gemma::gemma;
use rsllm::candle_mistral::mistral;
use rsllm::handle_long_string;
use rsllm::network_capture::{network_capture, NetworkCapture};
use rsllm::openai_api::{format_messages_for_llama2, stream_completion, Message, OpenAIRequest};
use rsllm::pipeline::{process_image, process_speech, send_to_ndi, MessageData, ProcessedData};
use rsllm::stable_diffusion::SDConfig;
use rsllm::stream_data::{
    get_pid_map, identify_video_pid, is_mpegts_or_smpte2110, parse_and_store_pat, process_packet,
    update_pid_map, Codec, PmtInfo, StreamData, Tr101290Errors, PAT_PID,
};
use rsllm::stream_data::{process_mpegts_packet, process_smpte2110_packet};
use rsllm::twitch_client::setup as twitch_setup;
use rsllm::{current_unix_timestamp_ms, hexdump, hexdump_ascii};
use rsllm::{get_stats_as_json, StatsType};
use serde_json::{self, json};
use std::collections::HashMap;
use std::env;
use std::io::Write;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tokio::sync::mpsc::{self};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    // Read .env file
    dotenv::dotenv().ok();

    // Initialize logging
    let _ = env_logger::try_init();

    // Parse command line arguments
    let args = Args::parse();

    // Set Rust log level with --loglevel if it is set
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

    let system_message = Message {
        role: "system".to_string(),
        content: args.system_prompt.to_string(),
    };

    let processed_data_store: Arc<Mutex<HashMap<usize, ProcessedData>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Channels for image and speech tasks
    let (pipeline_task_sender, mut pipeline_task_receiver) =
        mpsc::channel::<MessageData>(args.pipeline_concurrency * 2);

    let image_sem = Arc::new(Semaphore::new(args.image_concurrency));
    let speech_sem = Arc::new(Semaphore::new(args.speech_concurrency));
    let pipeline_sem = Arc::new(Semaphore::new(args.pipeline_concurrency));
    let ndi_sem = Arc::new(Semaphore::new(1));

    // Pipeline processing task for image and speech together as a single task
    let pipeline_processing_task = {
        let pipeline_sem = Arc::clone(&pipeline_sem);
        let processed_data_store = processed_data_store.clone();
        tokio::spawn(async move {
            while let Some(message_data) = pipeline_task_receiver.recv().await {
                let pipeline_sem = Arc::clone(&pipeline_sem);
                let processed_data_store = processed_data_store.clone();
                let message_data_clone = message_data.clone();
                let image_sem = Arc::clone(&image_sem);
                let speech_sem = Arc::clone(&speech_sem);
                let pipeline_sem = Arc::clone(&pipeline_sem);
                tokio::spawn(async move {
                    let _permit = pipeline_sem
                        .acquire()
                        .await
                        .expect("Failed to acquire pipeline semaphore permit");

                    let images =
                        process_image(message_data_clone.clone(), Arc::clone(&image_sem)).await;
                    let speech_data =
                        process_speech(message_data_clone.clone(), Arc::clone(&speech_sem)).await;
                    let mut store = processed_data_store.lock().await;

                    match store.entry(message_data_clone.paragraph_count) {
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(ProcessedData {
                                paragraph: message_data_clone.paragraph.clone(),
                                image_data: Some(images),
                                audio_data: Some(speech_data),
                                paragraph_count: message_data_clone.paragraph_count,
                                subtitle_position: message_data_clone.subtitle_position.clone(),
                                time_stamp: 0,
                            });
                        }
                        std::collections::hash_map::Entry::Occupied(mut e) => {
                            let entry = e.get_mut();
                            entry.image_data = Some(images);
                            entry.audio_data = Some(speech_data);
                        }
                    }
                });
            }
        })
    };

    // NDI sync task
    let processed_data_store_for_ndi = processed_data_store.clone();
    let args_for_ndi = args.clone();

    let running_processor_ndi = Arc::new(AtomicBool::new(true));
    let running_processor_clone = running_processor_ndi.clone();
    let ndi_sync_task = tokio::spawn(async move {
        let ndi_sem = Arc::clone(&ndi_sem);
        while running_processor_clone.load(Ordering::SeqCst) {
            let keys_to_remove = {
                let store = processed_data_store_for_ndi.lock().await;
                store
                    .iter()
                    .filter_map(|(&key, data)| {
                        if data.image_data.is_some() && data.audio_data.is_some() {
                            Some(key)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            };

            // sleep if there is no data to process
            if keys_to_remove.is_empty() {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                continue;
            }

            for key in keys_to_remove {
                if let Some(data) = processed_data_store_for_ndi.lock().await.remove(&key) {
                    send_to_ndi(data, &args_for_ndi, Arc::clone(&ndi_sem)).await;
                }
            }
        }
    });

    let mut llm_host = args.llm_host.clone();
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
    if args.ai_network_stats {
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
            if args.ai_network_stats {
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
                    break;
                }
            } else {
                // sleep for a while to avoid busy loop
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    });

    let twitch_auth = env::var("TWITCH_AUTH")
        .ok()
        .unwrap_or_else(|| "NO_AUTH_KEY".to_string());

    let running_processor_twitch = Arc::new(AtomicBool::new(true));
    if args.twitch_client {
        // Clone values before moving them into the closure
        let twitch_channel_clone = vec![args.twitch_channel.clone()];
        let twitch_username_clone = args.twitch_username.clone();
        let twitch_auth_clone = twitch_auth.clone(); // Assuming twitch_auth is clonable and you want to use it within the closure.

        // TODO: add mpsc channels for communication between the twitch setup and the main thread
        let running_processor_clone = running_processor_twitch.clone();
        let _twitch_handle = tokio::spawn(async move {
            info!(
                "Setting up Twitch channel {} for user {}",
                twitch_channel_clone.join(", "), // Assuming it's a Vec<String>
                twitch_username_clone
            );

            if twitch_auth == "NO_AUTH_KEY" {
                error!(
                    "Twitch Auth key is not set. Please set the TWITCH_AUTH environment variable."
                );
                std::process::exit(1);
            }

            match twitch_setup(
                twitch_username_clone.clone(),
                twitch_auth_clone,
                twitch_channel_clone.clone(),
                running_processor_clone,
            )
            .await
            {
                Ok(_) => {
                    info!(
                        "Twitch setup successful for channel {} username {}",
                        twitch_channel_clone.join(", "), // Assuming it's a Vec<String>
                        twitch_username_clone
                    );
                }
                Err(e) => {
                    error!(
                        "Error setting up Twitch channel {} for user {}: {}",
                        twitch_channel_clone.join(", "), // Assuming it's a Vec<String>
                        twitch_username_clone,
                        e
                    );
                }
            }
        });

        //TODO: put this at the end.
        // wait for the running_processor to be set to false
        /*if let Err(e) = twitch_handle.await {
        error!("Error waiting for Twitch channel: {}", e);
        }*/
    }
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
        let system_stats_json = if args.ai_os_stats {
            get_stats_as_json(StatsType::System).await
        } else {
            // Default input message
            json!({})
        };

        // Add the system stats to the messages
        if !args.ai_os_stats && !args.ai_network_stats {
            let query_clone = args.query.clone();

            let user_message = Message {
                role: "user".to_string(),
                content: query_clone.to_string(),
            };
            messages.push(user_message.clone());
        } else if args.ai_network_stats {
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
        } else if args.ai_os_stats {
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

        // Spawn a thread to run the LLM function, to keep the UI responsive streaming the response
        if !args.use_api && !args.use_openai {
            // Capture the start time for performance metrics
            let start = Instant::now();

            let chat_format = if args.candle_llm == "mistral" {
                // check if model_id includes the string "Instruct" within it
                if args.model_id.contains("Instruct") {
                    "llama2".to_string()
                } else {
                    "".to_string()
                }
            } else if args.candle_llm == "gemma" {
                if args.model_id == "7b-it" {
                    "google".to_string()
                } else if args.model_id == "2b-it" {
                    "google".to_string()
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
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
                        args.max_tokens as usize,
                        args.temperature as f64,
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
                        args.max_tokens as usize,
                        args.temperature as f64,
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
            let mut terminal_token_len = 0;
            let mut answers = Vec::new();
            let mut paragraphs: Vec<String> = Vec::new();
            let mut current_paragraph: Vec<String> = Vec::new();
            let mut paragraph_count = 0;
            let min_paragraph_len = args.sd_text_min; // Minimum length of a paragraph to generate an image

            // create uuid unique identifier for the output images
            let output_id = Uuid::new_v4().simple().to_string(); // Generates a UUID and converts it to a simple, hyphen-free string

            while let Some(received) = external_receiver.recv().await {
                token_count += 1;
                terminal_token_len += received.len();

                // Store the received token
                answers.push(received.clone());

                // If a newline is at the end of the token, process the accumulated paragraph for image generation
                if received.contains('\n') && !current_paragraph.is_empty()
                    || (current_paragraph.join("").len() > args.sd_max_length
                        && (received.contains('.')
                            || received.contains('?')
                            || received.contains('\n')
                            || received.contains('!'))
                        || (current_paragraph.join("").len()
                            >= (2.5 * args.sd_max_length as f32) as usize
                            && (received.contains(' '))))
                {
                    // Join the current paragraph tokens into a single String without adding extra spaces
                    if !current_paragraph.is_empty()
                        && current_paragraph.join("").len() > min_paragraph_len
                    {
                        // check if token has the new line character, split it at the new line into two parts, then put the first part onto
                        // the current paragraph and the second part into the answers and current_paragraph later after we store the current paragraph
                        // Safely handle split at the newline character
                        let mut split_pos = received.len();
                        for delimiter in ['\n', '.', ',', '?', '!'] {
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
                        let mut nl: bool = false;
                        if first.ends_with('\n') {
                            first = &first[..first.len() - 1];
                            nl = true;
                        } else if second.starts_with('\n') {
                            second = &second[1..];
                            nl = true;
                        }

                        // Store the first part of the split token into the current paragraph
                        current_paragraph.push(first.to_string());

                        let paragraph_text = current_paragraph.join(""); // Join without spaces as indicated
                        paragraphs.push(paragraph_text.clone());

                        // Token output to stdout in real-time
                        print!("{}", first);
                        if nl {
                            print!("\n");
                            terminal_token_len = 0;
                        } else if current_paragraph.len() >= 80 {
                            print!("\n");
                            terminal_token_len = 0;
                        }
                        std::io::stdout().flush().unwrap();

                        // Clear current paragraph for the next one
                        current_paragraph.clear(); // Clear current paragraph for the next one

                        // Store the second part of the split token into the answers and current_paragraph
                        current_paragraph.push(second.to_string());

                        // ** Start of TTS and Image Generation **
                        // Check if image generation is enabled and proceed
                        if args.sd_image || args.tts_enable || args.oai_tts || args.mimic3_tts {
                            // Clone necessary data for use in the async block
                            let paragraph_clone = paragraphs[paragraph_count].clone();
                            let output_id_clone = output_id.clone();
                            let mimic3_voice = args.mimic3_voice.clone().to_string();
                            let image_alignment = args.image_alignment.clone();
                            let subtitle_position = args.subtitle_position.clone();
                            let args = args.clone();

                            let pipeline_task_sender_clone = pipeline_task_sender.clone();

                            let mut sd_config = SDConfig::new();
                            sd_config.prompt = paragraph_clone;
                            sd_config.height = Some(args.sd_height);
                            sd_config.width = Some(args.sd_width);
                            sd_config.image_position = Some(image_alignment);
                            if args.sd_scaled_height > 0 {
                                sd_config.scaled_height = Some(args.sd_scaled_height);
                            }
                            if args.sd_scaled_width > 0 {
                                sd_config.scaled_width = Some(args.sd_scaled_width);
                            }

                            let args_clone = args.clone();
                            let mimic3_voice_clone = mimic3_voice.clone();
                            let subtitle_position_clone = subtitle_position.clone();

                            debug!("Generating images with prompt: {}", sd_config.prompt);

                            // Create MessageData for image task
                            let message_data_for_pipeline = MessageData {
                                paragraph: sd_config.prompt.clone(), // Clone for the image task
                                output_id: output_id_clone.clone(),
                                paragraph_count,
                                sd_config: sd_config.clone(), // Assuming SDConfig is set up previously and is cloneable
                                mimic3_voice: mimic3_voice_clone.clone(),
                                subtitle_position: subtitle_position_clone.clone(),
                                args: args_clone.clone(),
                            };

                            // For image tasks
                            pipeline_task_sender_clone
                                .send(message_data_for_pipeline)
                                .await
                                .expect("Failed to send image/speech pipeline task");
                        }
                        // ** End of TTS and Image Generation **

                        // Token output to stdout in real-time
                        print!("{}", second);
                        std::io::stdout().flush().unwrap();

                        paragraph_count += 1; // Increment paragraph count for the next paragraph
                    } else {
                        // store the token in the current paragraph
                        current_paragraph.push(received.clone());

                        // Call the function to handle the string if it exceeds 80 characters
                        handle_long_string(&received, &mut terminal_token_len);

                        std::io::stdout().flush().unwrap();
                    }
                } else {
                    // store the token in the current paragraph
                    current_paragraph.push(received.clone());

                    // Call the function to handle the string if it exceeds 80 characters
                    handle_long_string(&received, &mut terminal_token_len);

                    std::io::stdout().flush().unwrap();
                }
            }

            // Join the last paragraph tokens into a single String without adding extra spaces
            if current_paragraph.len() > 0 {
                // ** Start of TTS and Image Generation **
                // Check if image generation is enabled and proceed
                if args.sd_image || args.tts_enable || args.oai_tts || args.mimic3_tts {
                    // Clone necessary data for use in the async block
                    let paragraph_text = current_paragraph.join(""); // Join without spaces as indicated
                    let paragraph_clone = paragraph_text.clone();
                    let output_id_clone = output_id.clone();
                    let mimic3_voice = args.mimic3_voice.clone().to_string();
                    let image_alignment = args.image_alignment.clone();
                    let subtitle_position = args.subtitle_position.clone();
                    let args = args.clone();

                    let pipeline_task_sender_clone = pipeline_task_sender.clone();

                    let mut sd_config = SDConfig::new();
                    sd_config.prompt = paragraph_clone;
                    sd_config.height = Some(args.sd_height);
                    sd_config.width = Some(args.sd_width);
                    sd_config.image_position = Some(image_alignment);
                    if args.sd_scaled_height > 0 {
                        sd_config.scaled_height = Some(args.sd_scaled_height);
                    }
                    if args.sd_scaled_width > 0 {
                        sd_config.scaled_width = Some(args.sd_scaled_width);
                    }

                    let args_clone = args.clone();
                    let mimic3_voice_clone = mimic3_voice.clone();
                    let subtitle_position_clone = subtitle_position.clone();

                    // Create MessageData for image task
                    let message_data_for_pipeline = MessageData {
                        paragraph: sd_config.prompt.clone(), // Clone for the image task
                        output_id: output_id_clone.clone(),
                        paragraph_count,
                        sd_config: sd_config.clone(), // Assuming SDConfig is set up previously and is cloneable
                        mimic3_voice: mimic3_voice_clone.clone(),
                        subtitle_position: subtitle_position_clone.clone(),
                        args: args_clone.clone(),
                    };

                    // For image tasks
                    pipeline_task_sender_clone
                        .send(message_data_for_pipeline)
                        .await
                        .expect("Failed to send last audio/speech pipeline task");
                }
                // ** End of TTS and Image Generation **

                // Token output to stdout in real-time
                print!("{}", current_paragraph.join(""));
                std::io::stdout().flush().unwrap();

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
        } else {
            // Stream API Completion
            let stream = !args.no_stream;
            let open_ai_request = OpenAIRequest {
                model: &args.model,
                max_tokens: &args.max_tokens, // add this field to the request struct
                messages: messages.clone(),
                temperature: &args.temperature, // add this field to the request struct
                top_p: &args.top_p,             // add this field to the request struct
                presence_penalty: &args.presence_penalty, // add this field to the request struct
                frequency_penalty: &args.frequency_penalty, // add this field to the request struct
                stream: &stream,
            };

            // Directly await the future; no need for an explicit runtime block
            let answers = stream_completion(
                open_ai_request,
                &openai_key.clone(),
                &llm_host,
                &args.llm_path,
                args.debug_inline,
                args.show_output_errors,
                args.break_line_length,
                args.sd_image,
                args.ndi_images,
                args.hardsub_font_size,
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
            if args.ai_network_stats {
                network_capture_config
                    .running
                    .store(false, Ordering::SeqCst);
            }

            // stop the running threads
            running_processor.store(false, Ordering::SeqCst);
            running_processor_ndi.store(false, Ordering::SeqCst);
            running_processor_twitch.store(false, Ordering::SeqCst);

            // Await the completion of background tasks
            let _ = processing_handle.await;
            let _ = pipeline_processing_task.await;
            let _ = ndi_sync_task.await;

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
