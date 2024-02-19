use image::{ImageBuffer, Rgb};
use ndi_sdk::send::{SendColorFormat, SendInstance};
use ndi_sdk::NDIInstance;
use std::io::Result;

pub fn send_images_over_ndi(images: Vec<ImageBuffer<Rgb<u8>, Vec<u8>>>) -> Result<()> {
    let instance: NDIInstance = ndi_sdk::load().expect("Failed to construct NDI instance");

    let mut sender = instance
        .create_send_instance("Rust NDI Image Stream".to_string(), false, false)
        .expect("Expected sender instance to be created");

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

        sender.send_video(frame);

        println!("Image sent over NDI.");
    }

    Ok(())
}

fn convert_rgb_to_rgba(image_buffer: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> Vec<u8> {
    image_buffer
        .pixels()
        .flat_map(|pixel| {
            let Rgb(data) = *pixel;
            vec![data[0], data[1], data[2], 255] // Adding full alpha value
        })
        .collect()
}
