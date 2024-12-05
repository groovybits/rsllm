#[cfg(not(feature = "fonts"))]
use crate::convert_rgb_to_rgba;
#[cfg(feature = "fonts")]
use crate::convert_rgb_to_rgba_with_text;
use image::{ImageBuffer, Rgb};
#[cfg(feature = "ndi")]
use ndi_sdk_rsllm::send::{SendColorFormat, SendInstance};
#[cfg(feature = "ndi")]
use ndi_sdk_rsllm::NDIInstance;
use once_cell::sync::Lazy;
use std::io::Result;
use std::sync::Mutex;

// Use Mutex to ensure thread-safety for NDIInstance and SendInstance
#[cfg(feature = "ndi")]
static NDI_INSTANCE: Lazy<Mutex<NDIInstance>> = Lazy::new(|| {
    let instance = ndi_sdk_rsllm::load().expect("Failed to construct NDI instance");
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
        #[cfg(feature = "fonts")]
        let mut subtitle_height = height as i32 - (height as i32 / 3);
        #[cfg(feature = "fonts")]
        if subtitle_position == "top" {
            subtitle_height = 10;
        } else if subtitle_position == "mid-top" {
            subtitle_height = height as i32 - (height as i32 / 2) / 2;
        } else if subtitle_position == "center" || subtitle_position == "middle" {
            subtitle_height = height as i32 - (height as i32 / 2);
        } else if subtitle_position == "low-center" {
            subtitle_height = height as i32 - (height as i32 / 3);
        } else if subtitle_position == "mid-bottom" {
            subtitle_height = height as i32 - (height as i32 / 4);
        } else if subtitle_position == "bottom" {
            subtitle_height = height as i32 - (height as i32 / 5);
        } else {
            log::error!(
                "Invalid subtitle position '{}', using default position bottom as value {} instead.",
                subtitle_position,
                subtitle_height
            );
        }

        #[cfg(feature = "fonts")]
        let start_pos = ((font_size as i32 * 1) as i32, subtitle_height); // Text start position (x, y)

        #[cfg(feature = "fonts")]
        let rgba_buffer =
            convert_rgb_to_rgba_with_text(&image_buffer, subtitle, font_size, start_pos);
        #[cfg(not(feature = "fonts"))]
        let rgba_buffer = convert_rgb_to_rgba(&image_buffer);

        let frame = ndi_sdk_rsllm::send::create_ndi_send_video_frame(
            width as i32,
            height as i32,
            ndi_sdk_rsllm::send::FrameFormatType::Progressive,
        )
        .with_data(rgba_buffer, width as i32 * 4, SendColorFormat::Rgba)
        .build()
        .expect("Expected frame to be created");

        log::debug!("Video sending over NDI: frame size {}x{}", width, height);

        sender.send_video(frame);

        // sleep for amount of a 60 fps frame
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    Ok(())
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

    let frame = ndi_sdk_rsllm::send::create_ndi_send_audio_frame(no_channels, sample_rate)
        .with_data(samples, sample_rate)
        .build()
        .expect("Expected audio sample to be created");

    sender.send_audio(frame);

    Ok(())
}
