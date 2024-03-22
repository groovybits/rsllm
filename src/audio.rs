use minimp3::{Decoder, Frame};
use std::io::Cursor;
use std::io::Result;

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
