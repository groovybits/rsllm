use image::{ImageBuffer, Rgb, Rgba};
use imageproc::drawing::draw_text_mut;
use minimp3::{Decoder, Frame};
#[cfg(feature = "ndi")]
use ndi_sdk::send::{SendColorFormat, SendInstance};
#[cfg(feature = "ndi")]
use ndi_sdk::NDIInstance;
use once_cell::sync::Lazy;
use rusttype::{Font, Scale};
use std::io::Cursor;
use std::io::Result;
use std::sync::Mutex;

/// Converts WAV PCM data to f32 samples without explicitly handling error cases in the return type.
///
/// # Arguments
/// * `wav_data` - The bytes of a WAV file.
///
/// # Returns
/// A `Result` containing a `Vec<f32>` of normalized audio samples, or an `Error`.
pub fn wav_to_f32(wav_data: Vec<u8>) -> Result<Vec<f32>> {
    let cursor = Cursor::new(wav_data);
    let reader_result = hound::WavReader::new(cursor);

    // Check if the reader was successfully created
    let reader = match reader_result {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()), // In case of an error, return an empty vector to match the mp3_to_f32 strategy
    };

    // Depending on the sample format, process the samples differently
    let spec = reader.spec();
    let sample_format = spec.sample_format;
    let bits_per_sample = spec.bits_per_sample;

    let samples = match sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .filter_map(|result_sample| result_sample.ok()) // Convert Result<f32, hound::Error> to Option<f32>, and then filter_map will filter out the None values
            .collect(),

        hound::SampleFormat::Int => match bits_per_sample {
            16 => reader
                .into_samples::<i16>()
                .filter_map(|result_sample| result_sample.ok()) // Convert Result<i16, hound::Error> to Option<i16>
                .map(|sample| sample as f32 / i16::MAX as f32) // Normalize
                .collect(),

            24 => reader
                .into_samples::<i32>()
                .filter_map(|result_sample| result_sample.ok()) // Convert Result<i32, hound::Error> to Option<i32>
                .map(|sample| (sample >> 8) as f32 / i16::MAX as f32) // Shift and normalize for 24-bit stored in i32
                .collect(),

            // In case of an unsupported bit depth, return an empty vector
            _ => Vec::new(),
        },
    };

    Ok(samples)
}

pub fn mp3_to_f32(mp3_data: Vec<u8>) -> Result<Vec<f32>> {
    let cursor = Cursor::new(mp3_data);
    let mut decoder = Decoder::new(cursor);
    let mut samples_f32 = Vec::new();

    while let Ok(Frame { data, .. }) = decoder.next_frame() {
        for &sample in &data {
            // Convert each sample to f32; MP3 samples are typically s16.
            // Normalize the s16 sample to the range [-1.0, 1.0].
            let sample_f32 = sample as f32 / i16::MAX as f32;
            samples_f32.push(sample_f32);
        }
    }

    Ok(samples_f32)
}

// Use Mutex to ensure thread-safety for NDIInstance and SendInstance
#[cfg(feature = "ndi")]
static NDI_INSTANCE: Lazy<Mutex<NDIInstance>> = Lazy::new(|| {
    let instance = ndi_sdk::load().expect("Failed to construct NDI instance");
    Mutex::new(instance)
});

#[cfg(feature = "ndi")]
static NDI_SENDER: Lazy<Mutex<SendInstance>> = Lazy::new(|| {
    let instance = NDI_INSTANCE.lock().unwrap();
    let sender = instance
        .create_send_instance("RsLLM".to_string(), false, false)
        .expect("Expected sender instance to be created");
    Mutex::new(sender)
});

#[cfg(feature = "ndi")]
pub fn send_images_over_ndi(
    images: Vec<ImageBuffer<Rgb<u8>, Vec<u8>>>,
    subtitle: &str,
    font_size: f32,
    subtitle_position: &str,
) -> Result<()> {
    let mut sender = NDI_SENDER.lock().unwrap();

    for image_buffer in images {
        let width = image_buffer.width();
        let height = image_buffer.height();

        // adjust height depending on subtitle_postion as top, center, bottom with respect to the image height
        let mut subtitle_height = height as i32 - (height as i32 / 3);
        if subtitle_position == "top" {
            subtitle_height = 10;
        } else if subtitle_position == "mid-top" {
            subtitle_height = height as i32 - (height as i32 / 2) / 2;
        } else if subtitle_position == "center" || subtitle_position == "middle" {
            subtitle_height = height as i32 - (height as i32 / 2);
        } else if subtitle_position == "mid-bottom" {
            subtitle_height = height as i32 - (height as i32 / 3);
        } else if subtitle_position == "bottom" {
            subtitle_height = height as i32 - (height as i32 / 4);
        } else {
            log::error!(
                "Invalid subtitle position '{}', using default position bottome as value {} instead.",
                subtitle_position,
                subtitle_height
            );
        }

        let start_pos = ((font_size as i32 * 1) as i32, subtitle_height); // Text start position (x, y)

        let rgba_buffer =
            convert_rgb_to_rgba_with_text(&image_buffer, subtitle, font_size, start_pos);

        let frame = ndi_sdk::send::create_ndi_send_video_frame(
            width as i32,
            height as i32,
            ndi_sdk::send::FrameFormatType::Progressive,
        )
        .with_data(rgba_buffer, width as i32 * 4, SendColorFormat::Rgba)
        .build()
        .expect("Expected frame to be created");

        log::debug!("Video sending over NDI: frame size {}x{}", width, height);

        sender.send_video(frame);
    }

    Ok(())
}

// Helper function to wrap text into lines
#[cfg(feature = "ndi")]
fn wrap_text<'a>(text: &'a str, font: &Font, scale: Scale, max_width: i32) -> Vec<String> {
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
#[cfg(feature = "ndi")]
fn text_width(text: &str, font: &Font, scale: Scale) -> f32 {
    text.chars()
        .map(|c| font.glyph(c).scaled(scale).h_metrics().advance_width)
        .sum()
}

#[cfg(feature = "ndi")]
fn convert_rgb_to_rgba_with_text(
    image_buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    text: &str,
    font_size: f32,
    start_pos: (i32, i32), // Text start position (x, y)
) -> Vec<u8> {
    // Load the font. Ensure you have the font file at the specified path in your project directory.
    // The path should be relative to the root of your crate; for example, if your font is in the root,
    // the path could simply be "your_font.ttf".
    //
    //let font_data = include_bytes!("/System/Library/Fonts/Hiragino Sans GB.ttc");
    //
    //let font_data = include_bytes!("/System/Library/Fonts/Monaco.ttf");
    //
    let font_data = include_bytes!("../fonts/TrebuchetMSBold.ttf");

    let font = Font::try_from_bytes(font_data as &[u8]).expect("Error constructing Font");

    // Create a new ImageBuffer where we'll draw our text. Convert RGB to RGBA by adding an alpha channel.
    let mut image_rgba =
        ImageBuffer::from_fn(image_buffer.width(), image_buffer.height(), |x, y| {
            let pixel = image_buffer.get_pixel(x, y);
            Rgba([pixel[0], pixel[1], pixel[2], 255]) // Copy the RGB pixel and add full alpha
        });

    // Setup for drawing text
    let scale = Scale {
        x: font_size,
        y: font_size,
    }; // Adjust the font scale/size as needed
    let text_color = Rgba([255, 255, 255, 0xff]);

    // Wrap text and draw it
    let max_width = image_buffer.width() as i32 - (start_pos.0 as i32 * 2); // Max width for text before wrapping
    let wrapped_text = wrap_text(text, &font, scale, max_width);

    let mut current_height = start_pos.1;
    for line in wrapped_text {
        draw_text_mut(
            &mut image_rgba,
            text_color,
            start_pos.0,
            current_height,
            scale,
            &font,
            &line,
        );
        current_height += font_size as i32; // Adjust based on font size or measured line height
    }

    // Convert the modified RGBA image buffer back to a flat Vec<u8>
    image_rgba
        .pixels()
        .flat_map(|pixel| {
            let Rgba(data) = *pixel;
            vec![data[0], data[1], data[2], data[3]] // Return the RGBA values including the alpha channel
        })
        .collect()
}

#[cfg(feature = "ndi")]
pub fn send_audio_samples_over_ndi(
    samples: Vec<f32>,
    sample_rate: i32,
    no_channels: i32,
) -> Result<()> {
    let mut sender = NDI_SENDER.lock().unwrap();

    // Configuration validation (example)
    if sample_rate < 8000 || sample_rate > 192000 {
        log::error!("Unsupported sample rate: {}", sample_rate);
        return Ok(());
    }

    if no_channels < 1 || no_channels > 16 {
        log::error!("Unsupported channel count: {}", no_channels);
        return Ok(());
    }

    log::debug!(
        "Audio sending over NDI: {} samples at {} Hz",
        samples.len(),
        sample_rate
    );

    let frame = ndi_sdk::send::create_ndi_send_audio_frame(no_channels, sample_rate)
        .with_data(samples, sample_rate)
        .build()
        .expect("Expected audio sample to be created");

    sender.send_audio(frame);

    Ok(())
}
