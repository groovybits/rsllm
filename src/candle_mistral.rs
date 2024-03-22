#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

use anyhow::{Error as E, Result};
use safetensors::tensor::View;
use std::io::Write;
use tokio::sync::mpsc::{self, Sender};
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
use log::{debug, info};
use std::sync::Arc;
use tokenizers::Tokenizer;
use tokio::sync::Mutex;

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

    async fn run(&mut self, prompt: &str, sample_len: usize) -> Result<()> {
        let verbose_prompt: bool = false;
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
                if verbose_prompt {
                    println!("'{}'", t);
                    std::io::stdout().flush()?;
                }
            }
        }

        debug!("prompt: {:?}", prompt);

        let eos_token = match self.tokenizer.get_token("</s>") {
            Some(token) => token,
            None => anyhow::bail!("cannot find the </s> token"),
        };
        for index in 0..sample_len {
            let context_size = if index > 0 { 1 } else { tokens.len() };
            let start_pos = tokens.len().saturating_sub(context_size);
            let ctxt = &tokens[start_pos..];
            let input = Tensor::new(ctxt, &self.device)?.unsqueeze(0)?;
            //Model::Mistral7binstructV02(m) => m.forward(&input, start_pos)?,
            let logits = match &mut self.model {
                Model::Mistral(m) => match m.forward(&input, start_pos) {
                    Ok(logits) => logits,
                    Err(e) => return Err(anyhow::format_err!("Error during forward pass: {}", e)),
                },
                Model::Quantized(m) => match m.forward(&input, start_pos) {
                    Ok(logits) => logits,
                    Err(e) => return Err(anyhow::format_err!("Error during forward pass: {}", e)),
                },
            };

            let logits = logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32)?;

            // Check if logits are all zero
            let is_all_zero = logits.data().chunks_exact(4).all(|bytes| {
                let value = f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                value == 0.0
            });

            if is_all_zero {
                log::warn!("All logits are zero at index {}", index);

                // Retry up to 3 times
                let max_retries = 3;
                for retry in 1..=max_retries {
                    log::info!("Retrying ({}/{})", retry, max_retries);

                    let logits = match &mut self.model {
                        Model::Mistral(m) => match m.forward(&input, start_pos) {
                            Ok(logits) => logits,
                            Err(e) => {
                                log::error!("Error during retry: {}", e);
                                if retry == max_retries {
                                    return Err(anyhow::format_err!(
                                        "Failed to generate logits after {} retries: {}",
                                        max_retries,
                                        e
                                    ));
                                }
                                continue;
                            }
                        },
                        Model::Quantized(m) => match m.forward(&input, start_pos) {
                            Ok(logits) => logits,
                            Err(e) => {
                                log::error!("Error during retry: {}", e);
                                if retry == max_retries {
                                    return Err(anyhow::format_err!(
                                        "Failed to generate logits after {} retries: {}",
                                        max_retries,
                                        e
                                    ));
                                }
                                continue;
                            }
                        },
                    };

                    let logits = match logits.squeeze(0)?.squeeze(0)?.to_dtype(DType::F32) {
                        Ok(logits) => logits,
                        Err(e) => {
                            log::error!("Error during logits processing: {}", e);
                            return Err(anyhow::format_err!(
                                "Failed to process logits after {} retries: {}",
                                retry,
                                e
                            ));
                        }
                    };

                    let is_all_zero = logits.data().chunks_exact(4).all(|bytes| {
                        let value = f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                        value == 0.0
                    });

                    if !is_all_zero {
                        break;
                    }

                    if retry == max_retries {
                        return Err(anyhow::format_err!(
                            "All logits are zero after {} retries",
                            max_retries
                        ));
                    }
                }
            }

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

pub fn mistral(
    prompt: String,
    sample_len: usize,
    temperature: f64,
    quantized: bool,
    model_id: Option<String>,
    external_sender: Sender<String>,
) -> Result<()> {
    let cpu = false;
    let tracing = false;
    let use_flash_attn = false;
    let top_p: Option<f64> = None;
    let seed = rand::random();
    let revision: String = "main".to_string();
    let tokenizer_file: Option<String> = None;
    let weight_files: Option<String> = None;
    let repeat_penalty = 1.1;
    let repeat_last_n = (sample_len / 4) + prompt.len();

    let _guard = if tracing {
        let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
        tracing_subscriber::registry().with(chrome_layer).init();
        Some(guard)
    } else {
        None
    };
    debug!(
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
        Some(model_id) => {
            if model_id.is_empty() || model_id.to_string() == "auto" {
                if quantized {
                    "lmz/candle-mistral".to_string()
                } else {
                    "mistralai/Mistral-7B-Instruct-v0.2".to_string()
                }
            } else if model_id.to_lowercase() == "7b-it" {
                "mistralai/Mistral-7B-Instruct-v0.2".to_string()
            } else if model_id.to_lowercase() == "7b" {
                "mistralai/Mistral-7B-v0.1".to_string()
            } else {
                model_id.to_string()
            }
        }
        None => {
            if quantized {
                "lmz/candle-mistral".to_string()
            } else {
                "mistralai/Mistral-7B-Instruct-v0.2".to_string()
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
    info!("retrieved the files in {:?}", start.elapsed());
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

    info!("loaded the model in {:?}", start.elapsed());

    let (internal_sender, mut internal_receiver) = mpsc::channel(32768);

    // Pass both the internal and external senders to TextGeneration
    let pipeline = TextGeneration::new(
        model,
        tokenizer,
        seed,              // seed
        Some(temperature), // temp
        top_p,             // top_p
        repeat_penalty,    // repeat_penalty
        repeat_last_n,     // repeat_last_n
        &device,
        internal_sender,
    );

    let pipeline = Arc::new(Mutex::new(pipeline));

    // Start the text generation in a separate thread
    let pipeline_clone = pipeline.clone();
    let prompt_clone = prompt.clone();
    tokio::spawn(async move {
        let mut pipeline = pipeline_clone.lock().await;
        match pipeline.run(&prompt_clone, sample_len).await {
            Ok(_) => {}
            Err(e) => log::error!("Failed to run the pipeline: {}", e),
        }
    });

    // Set up a thread to listen on the internal receiver and forward messages to the external sender
    let external_sender_clone = external_sender.clone();
    tokio::spawn(async move {
        while let Some(token) = internal_receiver.recv().await {
            if let Err(e) = external_sender_clone.send(token).await {
                log::error!("Failed to send token externally: {}", e);
            }
        }
    });

    Ok(())
}
