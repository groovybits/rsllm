#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

use anyhow::{Error as E, Result};
use std::sync::mpsc::{self, Sender};
use std::thread;
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use candle_transformers::models::mistral::{Config, Model as Mistral};
use candle_transformers::models::quantized_mistral::Model as QMistral;

use candle_core::{DType, Device, Tensor};
use candle_examples::token_output_stream::TokenOutputStream;
use candle_nn::VarBuilder;
use candle_transformers::generation::LogitsProcessor;
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

enum Model {
    Mistral(Mistral),
    Quantized(QMistral),
}

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

    fn run(&mut self, prompt: &str, sample_len: usize) -> Result<()> {
        use std::io::Write;
        self.tokenizer.clear();
        let mut tokens = self
            .tokenizer
            .tokenizer()
            .encode(prompt, true)
            .map_err(E::msg)?
            .get_ids()
            .to_vec();
        for &t in tokens.iter() {
            if let Some(t) = self.tokenizer.next_token(t)? {
                self.internal_token_sender
                    .send(t.clone())
                    .expect("Failed to send token internally");
            }
        }
        std::io::stdout().flush()?;

        let eos_token = match self.tokenizer.get_token("</s>") {
            Some(token) => token,
            None => anyhow::bail!("cannot find the </s> token"),
        };
        let start_gen = std::time::Instant::now();
        for index in 0..sample_len {
            let context_size = if index > 0 { 1 } else { tokens.len() };
            let start_pos = tokens.len().saturating_sub(context_size);
            let ctxt = &tokens[start_pos..];
            let input = Tensor::new(ctxt, &self.device)?.unsqueeze(0)?;
            let logits = match &mut self.model {
                Model::Mistral(m) => m.forward(&input, start_pos)?,
                Model::Quantized(m) => m.forward(&input, start_pos)?,
            };
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
                    .expect("Failed to send token internally");
            }
        }

        Ok(())
    }
}

pub fn mistral(prompt: String, sample_len: usize, external_sender: Sender<String>) -> Result<()> {
    let cpu = false;
    let tracing = false;
    let use_flash_attn = false;
    let temperature = 0.8;
    let top_p = 0.9;
    let seed = 299792458;
    let model_id: Option<String> = None;
    let revision: String = "main".to_string();
    let tokenizer_file: Option<String> = None;
    let weight_files: Option<String> = None;
    let quantized = true;
    let repeat_penalty = 1.1;
    let repeat_last_n = 64;

    let _guard = if tracing {
        let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
        tracing_subscriber::registry().with(chrome_layer).init();
        Some(guard)
    } else {
        None
    };
    println!(
        "avx: {}, neon: {}, simd128: {}, f16c: {}",
        candle_core::utils::with_avx(),
        candle_core::utils::with_neon(),
        candle_core::utils::with_simd128(),
        candle_core::utils::with_f16c()
    );
    println!(
        "temp: {:.2} repeat-penalty: {:.2} repeat-last-n: {}",
        temperature, repeat_penalty, repeat_last_n
    );

    let start = std::time::Instant::now();
    let api = Api::new()?;
    let model_id = match model_id {
        Some(model_id) => model_id,
        None => {
            if quantized {
                "lmz/candle-mistral".to_string()
            } else {
                "mistralai/Mistral-7B-v0.1".to_string()
            }
        }
    };
    let repo = api.repo(Repo::with_revision(model_id, RepoType::Model, revision));
    let tokenizer_filename = match tokenizer_file {
        Some(file) => std::path::PathBuf::from(file),
        None => repo.get("tokenizer.json")?,
    };
    let filenames = match weight_files {
        Some(files) => files
            .split(',')
            .map(std::path::PathBuf::from)
            .collect::<Vec<_>>(),
        None => {
            if quantized {
                vec![repo.get("model-q4k.gguf")?]
            } else {
                candle_examples::hub_load_safetensors(&repo, "model.safetensors.index.json")?
            }
        }
    };
    println!("retrieved the files in {:?}", start.elapsed());
    let tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(E::msg)?;

    let start = std::time::Instant::now();
    let config = Config::config_7b_v0_1(use_flash_attn);
    let device = candle_examples::device(cpu)?;
    let (model, device) = if quantized {
        let filename = &filenames[0];
        let vb =
            candle_transformers::quantized_var_builder::VarBuilder::from_gguf(filename, &device)?;
        let model = QMistral::new(&config, vb)?;
        (Model::Quantized(model), device)
    } else {
        let dtype = if device.is_cuda() {
            DType::BF16
        } else {
            DType::F32
        };
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&filenames, dtype, &device)? };
        let model = Mistral::new(&config, vb)?;
        (Model::Mistral(model), device)
    };

    println!("loaded the model in {:?}", start.elapsed());

    let (internal_sender, internal_receiver) = mpsc::channel();

    // Pass both the internal and external senders to TextGeneration
    let mut pipeline = TextGeneration::new(
        model,
        tokenizer,
        0,         // seed
        Some(1.0), // temp
        Some(1.0), // top_p
        1.1,       // repeat_penalty
        64,        // repeat_last_n
        &device,
        internal_sender,
    );

    // Start the text generation in a separate thread
    thread::spawn(move || {
        pipeline
            .run(&prompt, sample_len)
            .expect("Failed to run the pipeline");
    });

    // Set up a thread to listen on the internal receiver and forward messages to the external sender
    let external_sender_clone = external_sender.clone();
    thread::spawn(move || {
        for token in internal_receiver {
            external_sender_clone
                .send(token)
                .expect("Failed to send token externally");
        }
    });

    // Additional logic as needed...
    Ok(())
}
