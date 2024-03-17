/*
    Image and Speech generation pipeline for NDI output
*/
use crate::adjust_caps;
use crate::args::Args;
use crate::candle_metavoice::metavoice;
use crate::mimic3_tts::tts as mimic3_tts;
use crate::mimic3_tts::Request as Mimic3TTSRequest;
#[cfg(feature = "ndi")]
use crate::ndi::send_audio_samples_over_ndi;
#[cfg(feature = "ndi")]
use crate::ndi::send_images_over_ndi;
use crate::openai_tts::tts as oai_tts;
use crate::openai_tts::Request as OAITTSRequest;
use crate::openai_tts::Voice as OAITTSVoice;
use crate::stable_diffusion::{sd, SDConfig};
use crate::ApiError;
use image::ImageBuffer;
use image::Rgb;
use log::debug;

// Message Data for Image and Speech generation functions to use
#[derive(Clone)]
pub struct MessageData {
    pub paragraph: String,
    pub output_id: String,
    pub paragraph_count: usize,
    pub sd_config: SDConfig,
    pub mimic3_voice: String,
    pub subtitle_position: String,
    pub args: Args,
    pub shutdown: bool,
}

// Function to process image generation
pub async fn process_image(data: MessageData) -> Vec<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    if data.args.sd_image {
        debug!("Generating images with prompt: {}", data.sd_config.prompt);
        match sd(data.sd_config).await {
            // Ensure `sd` function is async and await its result
            Ok(images) => {
                // Save images to disk
                if data.args.save_images {
                    for (index, image_bytes) in images.iter().enumerate() {
                        let image_file = format!(
                            "images/{}_{}_{}_.png",
                            data.output_id, data.paragraph_count, index
                        );
                        debug!(
                            "Image {} {}/{} saving to {}",
                            data.output_id, data.paragraph_count, index, image_file
                        );
                        image_bytes
                            .save(image_file)
                            .map_err(candle_core::Error::wrap)
                            .unwrap(); // And this as well
                    }
                }
                return images.clone();
            }
            Err(e) => {
                eprintln!("Error generating images for {}: {:?}", data.output_id, e);
            }
        }
    }
    // Return an empty vector of images of type Vec<ImageBuffer<Rgb<u8>, ...>>
    let images: Vec<ImageBuffer<Rgb<u8>, Vec<u8>>> = Vec::new();
    images
}

// Function to process speech generation
pub async fn process_speech(data: MessageData) -> Vec<u8> {
    if data.args.mimic3_tts || data.args.oai_tts || data.args.tts_enable {
        let input = data.sd_config.prompt.clone(); // Ensure this uses the appropriate text for TTS

        // use function to adjust caps pub fn adjust_caps(paragraph: &str) -> String {
        let input = adjust_caps(&input);

        let bytes_result = if data.args.oai_tts {
            // OpenAI TTS request
            let model = String::from("tts-1");
            let voice = OAITTSVoice::Nova;
            let oai_request = OAITTSRequest::new(model, input, voice);

            let openai_key =
                std::env::var("OPENAI_API_KEY").expect("TTS Thread: OPENAI_API_KEY not found");

            // Directly await the TTS operation without spawning a new thread
            oai_tts(oai_request, &openai_key).await
        } else if data.args.mimic3_tts {
            let api_request = Mimic3TTSRequest::new(input, data.mimic3_voice);
            // Mimic3 TTS request
            mimic3_tts(api_request)
                .await
                .map_err(|e| ApiError::Error(e.to_string()))
        } else {
            // Candle TTS request
            metavoice(input)
                .await
                .map_err(|e| ApiError::Error(e.to_string()))
        };

        match bytes_result {
            Ok(bytes) => {
                if data.args.ndi_audio {
                    return bytes.to_vec();
                } else {
                    // Example code to play audio directly, replace with your actual audio playback logic
                    println!("Playing TTS audio");
                    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
                    let sink = rodio::Sink::try_new(&stream_handle).unwrap();
                    let cursor = std::io::Cursor::new(bytes);
                    let source = rodio::Decoder::new_mp3(cursor).expect("Error decoding MP3");
                    sink.append(source);
                    sink.sleep_until_end();
                }
            }
            Err(e) => eprintln!("Error in TTS request: {}", e),
        }
    }
    // return empty samples_f32 if no TTS is enabled
    Vec::new()
}

// Struct to hold the processed audio and image data
#[derive(Clone)]
pub struct ProcessedData {
    pub paragraph: String,
    pub image_data: Option<Vec<ImageBuffer<Rgb<u8>, Vec<u8>>>>, // Updated to hold a vector of ImageBuffer
    pub audio_data: Option<Vec<u8>>,
    pub paragraph_count: usize,
    pub subtitle_position: String,
    pub time_stamp: u64,
    pub shutdown: bool,
}

// Function to send audio/video pairs to NDI
pub async fn send_to_ndi(processed_data: ProcessedData, args: &Args) {
    // check if args.subtitles is true, if so defined the processed_data.paragraph as a variable, if not have it be an empty string
    let subtitle = if args.subtitles {
        processed_data.paragraph
    } else {
        String::new()
    };

    if let Some(image_data) = processed_data.image_data {
        if args.ndi_images {
            #[cfg(feature = "ndi")]
            {
                debug!("Sending images over NDI");
                send_images_over_ndi(
                    image_data,
                    &subtitle,
                    args.hardsub_font_size,
                    &processed_data.subtitle_position,
                )
                .unwrap();
            }
        }
    }

    if let Some(audio_data) = processed_data.audio_data {
        if args.ndi_audio {
            #[cfg(feature = "ndi")]
            {
                let samples_result = if args.oai_tts {
                    crate::ndi::mp3_to_f32(audio_data)
                } else {
                    crate::ndi::wav_to_f32(audio_data)
                };

                if let Ok(samples_f32) = samples_result {
                    let sample_rate = if args.mimic3_tts { 22050 } else { 24000 };
                    let channels: i32 = 1;
                    let chunk_size = args.audio_chunk_size * sample_rate as f32 * channels as f32;
                    let delay_ms =
                        (chunk_size as f32 / channels as f32 / sample_rate as f32 * 1000.0) as u64;

                    debug!(
                        "Sending {} ms duration {} audio samples",
                        delay_ms, chunk_size
                    );

                    for chunk_samples in samples_f32.chunks(chunk_size as usize) {
                        let mut chunk_vec = chunk_samples.to_vec();

                        if chunk_samples.len() < chunk_size as usize {
                            chunk_vec.resize(chunk_size as usize, 0.0);
                        }

                        send_audio_samples_over_ndi(chunk_vec, sample_rate, channels)
                            .expect("Failed to send audio samples over NDI");

                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    }
                }
            }
        }
    }
}
