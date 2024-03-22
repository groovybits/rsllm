use crate::ApiError; // Assuming ApiError is defined in lib.rs and is in scope
use bytes::Bytes;
use log::debug;
use reqwest::Client;
use serde::Serialize;

const ENDPOINT: &str = "http://localhost:59125/api/tts"; // Mimic3 endpoint

#[derive(Serialize, Debug)]
pub struct Request {
    text: String,
    voice: String,
    #[serde(skip)]
    noise_scale: Option<f32>, // Defaults to 0.333 if None
    #[serde(skip)]
    noise_w: Option<f32>, // Defaults to 0.333 if None
    #[serde(skip)]
    length_scale: Option<f32>, // Defaults to 1.0 if None
    #[serde(skip)]
    ssml: Option<bool>, // Defaults to false if None
    #[serde(skip)]
    audio_target: Option<String>, // Defaults to "client" if None
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
            noise_scale: None,  // Default applied later
            noise_w: None,      // Default applied later
            length_scale: None, // Default applied later
            ssml: None,         // Default applied later
            audio_target: None, // Default applied later
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

    pub fn audio_target(mut self, target: String) -> Self {
        self.audio_target = Some(target);
        self
    }
}

pub async fn tts(req: Request) -> Result<Bytes, ApiError> {
    let client = Client::new();

    // Applying defaults where None is encountered
    let noise_scale = req.noise_scale.unwrap_or(0.333);
    let noise_w = req.noise_w.unwrap_or(0.333);
    let length_scale = req.length_scale.unwrap_or(1.0);
    let ssml = req.ssml.unwrap_or(false);
    let audio_target = req.audio_target.unwrap_or_else(|| "client".to_string());
    let text = req.text;

    let query_params = format!(
        "text={}&voice={}&noiseScale={}&noiseW={}&lengthScale={}&ssml={}&audioTarget={}",
        urlencoding::encode(&text),
        urlencoding::encode(&req.voice),
        noise_scale,
        noise_w,
        length_scale,
        ssml,
        urlencoding::encode(&audio_target),
    );

    let url = format!("{}?{}", ENDPOINT, query_params);

    debug!("Sending TTS GET request to URL: {}", url);

    let response = client.get(&url).send().await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                debug!("TTS request successful.");
                let audio_data = resp.bytes().await.map_err(ApiError::from)?;
                Ok(audio_data) // Returning the raw audio data as Bytes
            } else {
                let error_msg = format!("HTTP Error: {}", resp.status());
                Err(ApiError::Error(error_msg))
            }
        }
        Err(e) => Err(ApiError::from(e)),
    }
}
