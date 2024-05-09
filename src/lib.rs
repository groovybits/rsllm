/*
 * lib.rs
 * ------
 * Author: Chris Kennedy February @2024
 *
 * This file contains the main library for the stats and network capture modules
 * for RsLLM.
*/

pub mod args;
pub mod audio;
pub mod candle_metavoice;
pub mod candle_mistral;
pub mod mimic3_tts;
pub mod mpegts;
#[cfg(feature = "ndi")]
pub mod ndi;
pub mod network_capture;
pub mod openai_api;
pub mod openai_tts;
pub mod pipeline;
pub mod sd_automatic;
pub mod stable_diffusion;
pub mod stream_data;
pub mod system_stats;
pub mod twitch_client;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
pub use system_stats::{get_system_stats, SystemStats};
pub mod candle_gemma;
use image::{
    imageops::{resize, FilterType},
    ImageBuffer, Rgb, Rgba,
};
#[cfg(feature = "fonts")]
use imageproc::drawing::draw_text_mut;
#[cfg(feature = "fonts")]
use rusttype::{Font, Scale};
use std::io::Write;

#[derive(Debug)]
pub enum ApiError {
    Error(String),
    RequestError(reqwest::Error),
}

impl From<reqwest::Error> for ApiError {
    fn from(value: reqwest::Error) -> Self {
        ApiError::RequestError(value)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ApiError::Error(msg) => write!(f, "{}", msg),
            ApiError::RequestError(e) => write!(f, "Request error: {}", e),
        }
    }
}

/// Enum to determine the type of stats to fetch.
pub enum StatsType {
    System,
}

/// Fetches the requested stats and returns them as a JSON Value.
pub async fn get_stats_as_json(stats_type: StatsType) -> Value {
    match stats_type {
        StatsType::System => {
            let system_stats = get_system_stats();
            json!(system_stats)
        }
    }
}

// Function to get the current Unix timestamp in milliseconds
pub fn current_unix_timestamp_ms() -> Result<u64, &'static str> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|_| "System time is before the UNIX epoch")
}

// Print a hexdump of the packet
pub fn hexdump(packet_arc: &Arc<Vec<u8>>, packet_offset: usize, packet_len: usize) {
    let packet = &packet_arc[packet_offset..packet_offset + packet_len];
    // print in rows of 16 bytes
    let mut packet_dump = String::new();
    for (i, chunk) in packet.iter().take(packet_len).enumerate() {
        if i % 16 == 0 {
            packet_dump.push_str(&format!("\n{:04x}: ", i));
        }
        packet_dump.push_str(&format!("{:02x} ", chunk));
    }
    println!(
        "--- Packet Offset {} Packet Length {} ---\n{}\n---",
        packet_offset, packet_len, packet_dump
    );
}

// return a string of the packet in hex plus ascii representation after each hex line (16 bytes) with a | delimiter
pub fn hexdump_ascii(packet: &[u8], packet_offset: usize, packet_len: usize) -> String {
    // Assuming packet_offset and packet_len are correctly calculated within the slice's bounds
    let packet = &packet[packet_offset..packet_offset + packet_len];
    let mut packet_dump = String::new();
    for (i, &chunk) in packet.iter().enumerate() {
        if i % 16 == 0 {
            packet_dump.push_str(&format!("\n{:04x}: ", i));
        }
        packet_dump.push_str(&format!("{:02x} ", chunk));
        if i % 16 == 15 || i == packet.len() - 1 {
            // Adjust for last line
            packet_dump.push_str(" | ");
            let start = if i % 16 == 15 { i - 15 } else { i / 16 * 16 };
            for &ch in &packet[start..=i] {
                if ch >= 32 && ch <= 126 {
                    packet_dump.push(ch as char);
                } else {
                    packet_dump.push('.');
                }
            }
        }
    }
    packet_dump
}

/// Remove all caps from the provided string.
pub fn adjust_caps(paragraph: &str) -> String {
    paragraph
        .split_whitespace()
        .map(|word| {
            let mut c = word.chars();
            match c.next() {
                None => String::new(),
                Some(f) => {
                    f.to_uppercase().collect::<String>() + c.as_str().to_lowercase().as_str()
                }
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// Modifies the provided string if it exceeds 80 characters, splitting it according to specified delimiters,
/// and updates the `terminal_token_len` based on the operation performed.
///
/// # Arguments
///
/// * `received` - The string to potentially modify.
/// * `terminal_token_len` - The current length of the terminal token, to be updated.
pub fn handle_long_string(received: &str, terminal_token_len: &mut usize) {
    if *terminal_token_len >= 80 {
        std::io::stdout().flush().unwrap();

        // Initialize split position to the end of the string by default
        let mut split_pos = received.len();
        let mut found = false;
        for delimiter in ['\n', '.', ',', '?', '!'] {
            if let Some(pos) = received.find(delimiter) {
                // Adjust position to keep the delimiter with the first part, except for '\n'
                let end_pos = if delimiter == '\n' { pos } else { pos + 1 };
                split_pos = split_pos.min(end_pos);
                found = true;
                break;
            }
        }
        if split_pos == received.len() {
            if let Some(pos) = received.find(' ') {
                // Adjust position to keep the delimiter with the first part, except for '\n'
                let end_pos = pos + 1;
                split_pos = split_pos.min(end_pos);
                found = true;
            }
        }

        if found {
            let (first, second) = received.split_at(split_pos);
            print!("{}\n{}", first, second); // Use println! for simplicity to handle the newline
            *terminal_token_len = 0; //second.len(); // Update terminal_token_len with the length of the second part
        } else {
            print!("{}", received);
        }
        std::io::stdout().flush().unwrap();
    } else {
        print!("{}", received);
        std::io::stdout().flush().unwrap();
    }
}

/// Truncate the input text to the specified number of tokens.
/// If the number of tokens in the input text is less than or equal to the specified number of tokens,
/// the input text is returned as is. Otherwise, the input text is truncated to the specified number of tokens.
pub fn truncate_tokens(text: &str, max_tokens: usize) -> String {
    let mut tokens: Vec<String> = Vec::new();
    for token in text.split_whitespace() {
        if token.len() <= 4 {
            tokens.push(token.to_string());
        } else {
            let token_chars: Vec<char> = token.chars().collect();
            let chunks = token_chars.chunks(4);
            for chunk in chunks {
                let chunk_str: String = chunk.iter().collect();
                tokens.push(chunk_str);
            }
        }
    }

    if tokens.len() <= max_tokens {
        text.to_string()
    } else {
        tokens[..max_tokens].join(" ")
    }
}

pub fn count_tokens(text: &str) -> usize {
    let mut token_count = 0;
    for token in text.split_whitespace() {
        if token.len() <= 4 {
            token_count += 1;
        } else {
            let token_chars: Vec<char> = token.chars().collect();
            let chunks = token_chars.chunks(4);
            token_count += chunks.len();
        }
    }
    token_count
}

// Helper function to wrap text into lines
#[cfg(feature = "fonts")]
pub fn wrap_text<'a>(text: &'a str, font: &Font, scale: Scale, max_width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let space_width = font.glyph(' ').scaled(scale).h_metrics().advance_width;

    for word in text.split_whitespace() {
        let word_width = word
            .chars()
            .map(|c| font.glyph(c).scaled(scale).h_metrics().advance_width)
            .sum::<f32>();
        if current_line.is_empty()
            || text_width(&current_line, font, scale) + space_width + word_width <= max_width as f32
        {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = String::from(word);
        }
    }

    // go through lines and break any that exceed our max length into smaller lines and adjust proceeding lines
    // force break without wrap_text function since we are not breaking by spaces but by characters instead
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if text_width(line, font, scale) > max_width as f32 {
            // break line into smaller lines by character not by spaces
            let mut new_lines = Vec::new();
            let mut current_line = String::new();
            for c in line.chars() {
                let char_width = font.glyph(c).scaled(scale).h_metrics().advance_width;
                if text_width(&current_line, font, scale) + char_width <= max_width as f32 {
                    current_line.push(c);
                } else {
                    new_lines.push(current_line);
                    current_line = String::from(c);
                }
            }
        }
        i += 1;
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

// Helper function to calculate text width
#[cfg(feature = "fonts")]
pub fn text_width(text: &str, font: &Font, scale: Scale) -> f32 {
    text.chars()
        .map(|c| font.glyph(c).scaled(scale).h_metrics().advance_width)
        .sum()
}

#[cfg(feature = "fonts")]
pub fn convert_rgb_to_rgba_with_text(
    image_buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    text: &str,
    font_size: f32,
    start_pos: (i32, i32),
) -> Vec<u8> {
    let font_data = include_bytes!("../fonts/TrebuchetMSBold.ttf");
    let font = Font::try_from_bytes(font_data as &[u8]).expect("Error constructing Font");

    let mut image_rgba =
        ImageBuffer::from_fn(image_buffer.width(), image_buffer.height(), |x, y| {
            let pixel = image_buffer.get_pixel(x, y);
            Rgba([pixel[0], pixel[1], pixel[2], 255])
        });

    let scale = Scale {
        x: font_size,
        y: font_size,
    };
    let text_color = Rgba([255, 255, 255, 0xff]);
    let shadow_color = Rgba([0, 0, 0, 255]);
    let shadow_top_offset = 2; // Shadow offset in pixels
    let shadow_bottom_offset = 4; // Shadow offset in pixels

    // Draw bottom shadow
    let wrapped_text_shadow = wrap_text(
        text,
        &font,
        scale,
        image_buffer.width() as i32 - start_pos.0 * 2,
    );
    let mut current_height_bottom_shadow = start_pos.1 + shadow_bottom_offset / 2;
    for line in &wrapped_text_shadow {
        draw_text_mut(
            &mut image_rgba,
            shadow_color,
            start_pos.0 + shadow_bottom_offset,
            current_height_bottom_shadow,
            scale,
            &font,
            line,
        );
        current_height_bottom_shadow += font_size as i32;
    }

    // Draw top shadow
    let wrapped_text_shadow = wrap_text(
        text,
        &font,
        scale,
        image_buffer.width() as i32 - start_pos.0 * 2,
    );
    let mut current_height_top_shadow = start_pos.1 - shadow_top_offset / 2;
    for line in &wrapped_text_shadow {
        draw_text_mut(
            &mut image_rgba,
            shadow_color,
            start_pos.0 - shadow_top_offset,
            current_height_top_shadow,
            scale,
            &font,
            line,
        );
        current_height_top_shadow += font_size as i32;
    }

    // Draw text
    let wrapped_text = wrap_text(
        text,
        &font,
        scale,
        image_buffer.width() as i32 - start_pos.0 * 2,
    );
    let mut current_height = start_pos.1;
    for line in &wrapped_text {
        draw_text_mut(
            &mut image_rgba,
            text_color,
            start_pos.0,
            current_height,
            scale,
            &font,
            line,
        );
        current_height += font_size as i32;
    }

    image_rgba
        .pixels()
        .flat_map(|pixel| {
            let Rgba(data) = *pixel;
            vec![data[0], data[1], data[2], data[3]]
        })
        .collect()
}

#[cfg(not(feature = "fonts"))]
pub fn convert_rgb_to_rgba(image_buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> Vec<u8> {
    let image_rgba = ImageBuffer::from_fn(image_buffer.width(), image_buffer.height(), |x, y| {
        let pixel = image_buffer.get_pixel(x, y);
        Rgba([pixel[0], pixel[1], pixel[2], 255])
    });

    image_rgba
        .pixels()
        .flat_map(|pixel| {
            let Rgba(data) = *pixel;
            vec![data[0], data[1], data[2], data[3]]
        })
        .collect()
}

pub async fn clean_tts_input(input: String) -> String {
    // remove strings of periods anywhere within the input text and replace with a single period.
    // do it in a loop
    let mut input = input.clone();
    while input.contains("..") {
        input = input.replace("..", ".");
    }

    // remove <|im_end|> string from input and replace with ""
    let input = input.replace("<|im_end|>", "");

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

    input
}

pub fn scale_image(
    image: ImageBuffer<Rgb<u8>, Vec<u8>>,
    new_width: Option<u32>,
    new_height: Option<u32>,
    image_position: Option<String>,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    if let (Some(target_width), Some(target_height)) = (new_width, new_height) {
        if target_width == 0 || target_height == 0 {
            return image;
        }

        let (orig_width, orig_height) = image.dimensions();
        let scale = (target_width as f32 / orig_width as f32)
            .min(target_height as f32 / orig_height as f32);
        let scaled_width = (orig_width as f32 * scale).round() as u32;
        let scaled_height = (orig_height as f32 * scale).round() as u32;

        // Scale the image while preserving the aspect ratio.
        let scaled_image = resize(&image, scaled_width, scaled_height, FilterType::Lanczos3);

        // Create a new image with the target dimensions filled with black pixels.
        let mut new_image = ImageBuffer::from_pixel(target_width, target_height, Rgb([0, 0, 0]));

        // Calculate the offsets to position the scaled image based on image_position.
        let x_offset = match image_position.as_deref() {
            Some("left") => 0,
            Some("right") => target_width - scaled_width,
            _ => (target_width - scaled_width) / 2, // Default to center if it's not "left" or "right"
        };
        let y_offset = (target_height - scaled_height) / 2;

        // Copy the scaled image onto the new image at the calculated offset.
        for (x, y, pixel) in scaled_image.enumerate_pixels() {
            // Ensure the pixel is within the bounds of the target image dimensions.
            if x + x_offset < target_width && y + y_offset < target_height {
                new_image.put_pixel(x + x_offset, y + y_offset, *pixel);
            }
        }

        new_image
    } else {
        // Return the original image if dimensions are not specified.
        image
    }
}
