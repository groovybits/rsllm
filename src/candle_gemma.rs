#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

use anyhow::{Error as E, Result};
use log::{debug, info};
use std::io::Write;

use candle_transformers::models::gemma::{Config, Model};
use tokio::sync::mpsc::Sender;

use candle_core::{DType, Device, Tensor};
use candle_examples::token_output_stream::TokenOutputStream;
use candle_nn::VarBuilder;
use candle_transformers::generation::LogitsProcessor;
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

struct TextGeneration {
    model: Model,
    device: Device,
    tokenizer: TokenOutputStream,
    logits_processor: LogitsProcessor,
    repeat_penalty: f32,
    repeat_last_n: usize,
    internal_token_sender: Sender<String>,
}

impl TextGeneration {
    #[allow(clippy::too_many_arguments)]
    fn new(
        model: Model,
        tokenizer: Tokenizer,
        seed: u64,
        temp: Option<f64>,
        top_p: Option<f64>,
        repeat_penalty: f32,
        repeat_last_n: usize,
        device: &Device,
        internal_token_sender: Sender<String>,
    ) -> Self {
        let logits_processor = LogitsProcessor::new(seed, temp, top_p);
        Self {
            model,
            tokenizer: TokenOutputStream::new(tokenizer),
            logits_processor,
            repeat_penalty,
            repeat_last_n,
            device: device.clone(),
            internal_token_sender,
        }
    }

    async fn run(&mut self, prompt: &str, sample_len: usize) -> Result<()> {
        self.tokenizer.clear();
        let mut tokens = self
            .tokenizer
            .tokenizer()
            .encode(prompt, true)
            .map_err(E::msg)?
            .get_ids()
            .to_vec();

        debug!("prompt: {:?}", prompt);

        let eos_token = match self.tokenizer.get_token("<eos>") {
            Some(token) => token,
            None => anyhow::bail!("cannot find the <eos> token"),
        };
        for index in 0..sample_len {
            let context_size = if index > 0 { 1 } else { tokens.len() };
            let start_pos = tokens.len().saturating_sub(context_size);
            let ctxt = &tokens[start_pos..];
            let input = Tensor::new(ctxt, &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, start_pos)?;
            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;
            let logits = if self.repeat_penalty == 1. {
                logits
            } else {
                let start_at = tokens.len().saturating_sub(self.repeat_last_n);
                candle_transformers::utils::apply_repeat_penalty(
                    &logits,
                    self.repeat_penalty,
                    &tokens[start_at..],
                )?
            };

            let next_token = self.logits_processor.sample(&logits)?;
            tokens.push(next_token);
            if next_token == eos_token {
                break;
            }
            if let Some(t) = self.tokenizer.next_token(next_token)? {
                self.internal_token_sender
                    .send(t.clone())
                    .await
                    .expect("Failed to send token internally");
            }
        }

        Ok(())
    }
}

pub fn gemma(
    prompt: String,
    sample_len: usize,
    temperature: f64,
    _quantized: bool,
    model_id: Option<String>,
    external_sender: Sender<String>,
) -> Result<()> {
    let cpu = false;
    let tracing = false;
    let top_p: Option<f64> = None;
    let seed = rand::random();
    let revision: String = "main".to_string();
    let tokenizer_file: Option<String> = None;
    let config_file: Option<String> = None;
    let weight_files: Option<String> = None;
    let repeat_penalty = 1.1;
    let repeat_last_n = prompt.len();

    use tracing_chrome::ChromeLayerBuilder;
    use tracing_subscriber::prelude::*;

    let _guard = if tracing {
        let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
        tracing_subscriber::registry().with(chrome_layer).init();
        Some(guard)
    } else {
        None
    };
    info!(
        "avx: {}, neon: {}, simd128: {}, f16c: {}",
        candle_core::utils::with_avx(),
        candle_core::utils::with_neon(),
        candle_core::utils::with_simd128(),
        candle_core::utils::with_f16c()
    );
    info!(
        "temp: {:.2} repeat-penalty: {:.2} repeat-last-n: {}",
        temperature, repeat_penalty, repeat_last_n
    );

    let start = std::time::Instant::now();
    let api = Api::new()?;
    let model_id = match &model_id {
        Some(model_id) => match model_id.as_str() {
            "7b" => "google/gemma-7b".to_string(),
            "7b-it" => "google/gemma-7b-it".to_string(),
            "2b" => "google/gemma-2b".to_string(),
            "2b-it" => "google/gemma-2b-it".to_string(),
            "auto" => "google/gemma-2b-it".to_string(),
            _ => model_id.to_string(),
        },
        None => "google/gemma-2b-it".to_string(),
    };
    let repo = api.repo(Repo::with_revision(model_id, RepoType::Model, revision));
    let tokenizer_filename = match tokenizer_file {
        Some(file) => std::path::PathBuf::from(file),
        None => repo.get("tokenizer.json")?,
    };
    let config_filename = match config_file {
        Some(file) => std::path::PathBuf::from(file),
        None => repo.get("config.json")?,
    };
    let filenames = match weight_files {
        Some(files) => files
            .split(',')
            .map(std::path::PathBuf::from)
            .collect::<Vec<_>>(),
        None => candle_examples::hub_load_safetensors(&repo, "model.safetensors.index.json")?,
    };
    info!("retrieved the files in {:?}", start.elapsed());
    let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(E::msg)?;
    let config: Config = serde_json::from_reader(std::fs::File::open(config_filename)?)?;

    let start = std::time::Instant::now();
    let device = candle_examples::device(cpu)?;
    let dtype = if device.is_cuda() {
        DType::BF16
    } else {
        DType::F32
    };
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&filenames, dtype, &device)? };
    let model = Model::new(&config, vb)?;

    info!("loaded the model in {:?}", start.elapsed());

    let (internal_sender, mut internal_receiver) = tokio::sync::mpsc::channel::<String>(32); // Example buffer size

    let mut pipeline = TextGeneration::new(
        model,
        tokenizer,
        seed,
        Some(temperature),
        top_p,
        repeat_penalty,
        repeat_last_n,
        &device,
        internal_sender,
    );

    // Start the text generation in a separate thread
    tokio::spawn(async move {
        pipeline
            .run(&prompt, sample_len)
            .await
            .expect("Failed to run the pipeline");
    });

    // Set up a thread to listen on the internal receiver and forward messages to the external sender
    let external_sender_clone = external_sender.clone();
    tokio::spawn(async move {
        while let Some(token) = internal_receiver.recv().await {
            external_sender_clone
                .send(token)
                .await
                .expect("Failed to send token externally");
        }
    });

    Ok(())
}
