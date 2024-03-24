/*
    Image and Speech generation pipeline for NDI output
*/
use crate::adjust_caps;
use crate::args::Args;
#[cfg(feature = "ndi")]
use crate::audio::{mp3_to_f32, wav_to_f32};
#[cfg(feature = "metavoice")]
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
    pub last_message: bool,
}

// Function to process image generation
pub async fn process_image(mut data: MessageData) -> Vec<ImageBuffer<Rgb<u8>, Vec<u8>>> {
    // truncate tokens for sd_config.prompt
    data.sd_config.prompt = crate::truncate_tokens(&data.sd_config.prompt, data.args.sd_text_min);
    if data.args.sd_image {
        debug!("Generating images with prompt: {}", data.sd_config.prompt);
        let sd_clone = sd.clone();
        match sd_clone(data.sd_config).await {
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
                eprintln!("\nError generating images for {}: {:?}", data.output_id, e);
            }
        }
    }
    // Return an empty vector of images of type Vec<ImageBuffer<Rgb<u8>, ...>>
    let images: Vec<ImageBuffer<Rgb<u8>, Vec<u8>>> = Vec::new();
    images
}

// Function to process speech generation
pub async fn process_speech(data: MessageData) -> Vec<u8> {
    if data.args.mimic3_tts || data.args.oai_tts || data.args.tts_enable || data.args.metavoice_tts
    {
        let input = data.paragraph.clone(); // Ensure this uses the appropriate text for TTS

        // use function to adjust caps pub fn adjust_caps(paragraph: &str) -> String {
        let input = adjust_caps(&input);

        // remove all extra spaces besides 1 space between words, if all spaces left then reduce to '"
        let input = input
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ")
            .replace(" .", ".")
            .replace(" ,", ",")
            .replace(" ?", "?")
            .replace(" !", "!")
            .replace(" :", ":")
            .replace(" ;", ";");

        // remove any special characters from the text except for normal punctuation ./,;:?
        let input = input
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation())
            .collect::<String>();

        // split into sentences and check if any begin with special characters, remove them
        let input = input
            .split('.')
            .map(|s| {
                let s = s.trim();
                if s.starts_with(|c: char| c.is_ascii_punctuation()) {
                    &s[1..]
                } else {
                    s
                }
            })
            .collect::<Vec<&str>>()
            .join(". ");

        // remove any non ascii characters from the ending of the input text
        let input = input
            .chars()
            .take_while(|c| c.is_ascii())
            .collect::<String>();

        // loop removing end punctuation until no more
        let mut input = input.clone();
        while input.ends_with(|c: char| !c.is_alphanumeric()) {
            input = input
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .to_string();
        }

        // remove strings of periods anywhere within the input text and replace with a single period.
        // do it in a loop
        let mut input = input.clone();
        while input.contains("..") {
            input = input.replace("..", ".");
        }

        // check if input is "" empty and if so return here an empty Vec<u8>
        if input.is_empty() {
            return Vec::new();
        }

        debug!("\nTTS Speech text input: {}", input);

        let bytes_result = if data.args.oai_tts {
            // OpenAI TTS request
            let model = String::from("tts-1");
            let voice = OAITTSVoice::Nova;
            let oai_request = OAITTSRequest::new(model, input, voice);

            let openai_key =
                std::env::var("OPENAI_API_KEY").expect("TTS Thread: OPENAI_API_KEY not found");

            // Directly await the TTS operation without spawning a new thread
            oai_tts(oai_request, &openai_key).await
        } else if data.args.mimic3_tts || data.args.tts_enable {
            let api_request = Mimic3TTSRequest::new(input, data.mimic3_voice);
            // Mimic3 TTS request
            mimic3_tts(api_request)
                .await
                .map_err(|e| ApiError::Error(e.to_string()))
        } else if data.args.metavoice_tts {
            // Candle TTS request
            #[cfg(feature = "metavoice")]
            {
                match metavoice(input).await {
                    Ok(bytes) => return bytes.to_vec(),
                    Err(e) => {
                        eprintln!("Metavoice TTS error: {}", e);
                        return Vec::new(); // Return an empty Vec<u8> in case of an error
                    }
                }
            }

            #[cfg(not(feature = "metavoice"))]
            {
                eprintln!("Metavoice feature not enabled");
                return Vec::new(); // Return an empty Vec<u8> if the feature is not enabled
            }
        } else {
            Err(ApiError::Error("TTS type not implemented".to_string()))
        };

        match bytes_result {
            Ok(bytes) => {
                if data.args.ndi_audio {
                    return bytes.to_vec();
                } else {
                    // Example code to play audio directly, replace with your actual audio playback logic
                    // TODO: Split out into the audio crate
                    #[cfg(feature = "audioplayer")]
                    {
                        println!("Playing TTS audio");
                        let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
                        let sink = rodio::Sink::try_new(&stream_handle).unwrap();
                        let cursor = std::io::Cursor::new(bytes);
                        let source = rodio::Decoder::new_mp3(cursor).expect("Error decoding MP3");
                        sink.append(source);
                        sink.sleep_until_end();
                    }
                    #[cfg(not(feature = "audioplayer"))]
                    log::info!("Feature rodio isn't enabled for audio playback");
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
    pub completed: bool,
    pub last_message: bool,
}

// Function to send audio/video pairs to NDI
#[cfg(feature = "ndi")]
pub async fn send_to_ndi(processed_data: ProcessedData, args: &Args) {
    // check if args.subtitles is true, if so defined the processed_data.paragraph as a variable, if not have it be an empty string
    let subtitle = if args.subtitles {
        processed_data.paragraph
    } else {
        String::new()
    };

    if let Some(image_data) = processed_data.image_data {
        if args.ndi_images {
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
            let samples_result = if args.oai_tts {
                mp3_to_f32(audio_data)
            } else {
                wav_to_f32(audio_data)
            };

            if let Ok(mut samples_f32) = samples_result {
                let sample_rate = if args.mimic3_tts { 22050 } else { 24000 };
                let channels: i32 = 1;
                let chunk_size = args.audio_chunk_size * sample_rate as f32 * channels as f32;

                let delay_ms =
                    (chunk_size as f32 / channels as f32 / sample_rate as f32 * 1000.0) as u64;

                // Calculate the number of samples needed for 3 seconds of silence
                let silence_duration = 3.0; // Duration of silence in seconds
                let silence_samples = (silence_duration * sample_rate as f32) as usize;

                // Create a vector of silent samples
                let silence_vec = vec![0.0; silence_samples];

                // Prepend the silence to the audio samples
                samples_f32.splice(0..0, silence_vec.clone());

                // make sure the last chunk is aligned to the chunk size
                let last_chunk_size = samples_f32.len() as f32 % chunk_size;
                let last_chunk_size = if last_chunk_size == 0.0 {
                    chunk_size
                } else {
                    last_chunk_size
                };
                // Append silence to the last chunk to make it the same size as the other chunks
                let silence_samples = (chunk_size - last_chunk_size) as usize;
                let silence_vec = vec![0.0; silence_samples];
                samples_f32.extend(silence_vec);

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
