use crate::ApiError; // Assuming ApiError is defined in lib.rs and is in scope
use bytes::Bytes;
use log::debug;
use reqwest::Client;
use serde::Serialize;

const ENDPOINT: &str = "http://localhost:59125/api/tts"; // Mimic3 endpoint

#[derive(Serialize)]
pub struct Request {
    text: String,
    voice: String,
    noise_scale: Option<f32>,
    noise_w: Option<f32>,
    length_scale: Option<f32>,
    ssml: Option<bool>,
}
/*
params = {
            'text': text,
            'voice': voice or 'en_US/cmu-arctic_low#slt',
            'noiseScale': noise_scale or '0.333',
            'noiseW': noise_w or '0.333',
            'lengthScale': length_scale or '1.5',
            'ssml': ssml or 'false',
            'audioTarget': audio_target or 'client'
        }
        */

impl Request {
    pub fn new(text: String, voice: String) -> Self {
        Request {
            text,
            voice,
            noise_scale: Some(0.333),
            noise_w: Some(0.333),
            length_scale: Some(1.0),
            ssml: Some(false),
        }
    }

    pub fn noise_scale(mut self, scale: f32) -> Self {
        self.noise_scale = Some(scale);
        self
    }

    pub fn noise_w(mut self, w: f32) -> Self {
        self.noise_w = Some(w);
        self
    }

    pub fn length_scale(mut self, scale: f32) -> Self {
        self.length_scale = Some(scale);
        self
    }

    pub fn ssml(mut self, ssml: bool) -> Self {
        self.ssml = Some(ssml);
        self
    }
}

pub async fn tts(req: Request) -> Result<Bytes, ApiError> {
    let client = Client::new();

    debug!("Sending TTS request with voice: {} to Mimic3", req.voice);

    let response = client.post(ENDPOINT).json(&req).send().await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                resp.bytes().await.map_err(ApiError::from)
            } else {
                let error_msg = format!("HTTP Error: {}", resp.status());
                Err(ApiError::Error(error_msg))
            }
        }
        Err(e) => Err(ApiError::from(e)),
    }
}
