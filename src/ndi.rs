use image::{ImageBuffer, Rgb};
#[cfg(feature = "ndi")]
use ndi_sdk::send::{SendColorFormat, SendInstance};
#[cfg(feature = "ndi")]
use ndi_sdk::NDIInstance;
use once_cell::sync::Lazy;
use std::io::Result;
use std::sync::Mutex;

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
pub fn send_images_over_ndi(images: Vec<ImageBuffer<Rgb<u8>, Vec<u8>>>) -> Result<()> {
    let mut sender = NDI_SENDER.lock().unwrap();

    for image_buffer in images {
        let width = image_buffer.width();
        let height = image_buffer.height();

        let rgba_buffer = convert_rgb_to_rgba(&image_buffer);

        let frame = ndi_sdk::send::create_ndi_send_video_frame(
            width as i32,
            height as i32,
            ndi_sdk::send::FrameFormatType::Progressive,
        )
        .with_data(rgba_buffer, width as i32 * 4, SendColorFormat::Rgba)
        .build()
        .expect("Expected frame to be created");

        println!("Video sending over NDI: frame size {}x{}", width, height);

        sender.send_video(frame);
    }

    Ok(())
}

#[cfg(feature = "ndi")]
fn convert_rgb_to_rgba(image_buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> Vec<u8> {
    image_buffer
        .pixels()
        .flat_map(|pixel| {
            let Rgb(data) = *pixel;
            vec![data[0], data[1], data[2], 255] // Adding full alpha value
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

    log::info!(
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
