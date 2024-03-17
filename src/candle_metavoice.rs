#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

#[cfg(feature = "accelerate")]
extern crate accelerate_src;

#[cfg(feature = "metavoice")]
use anyhow::{Error, Result};
#[cfg(feature = "metavoice")]
use bytes::Bytes;
#[cfg(feature = "metavoice")]
use std::io::Cursor;
#[cfg(feature = "metavoice")]
use std::io::Write;

#[cfg(feature = "metavoice")]
use candle_transformers::generation::LogitsProcessor;
#[cfg(feature = "metavoice")]
use candle_transformers::models::encodec;
#[cfg(feature = "metavoice")]
use candle_transformers::models::metavoice::{adapters, gpt, tokenizers, transformer};
#[cfg(feature = "metavoice")]
use candle_transformers::models::quantized_metavoice::transformer as qtransformer;

#[cfg(feature = "metavoice")]
use candle_core::{DType, IndexOp, Tensor};
#[cfg(feature = "metavoice")]
use candle_nn::VarBuilder;
#[cfg(feature = "metavoice")]
use hf_hub::api::sync::Api;
#[cfg(feature = "metavoice")]
use rand::Rng;
#[cfg(feature = "metavoice")]
use rand::{distributions::Distribution, SeedableRng};

pub const ENCODEC_NTOKENS: u32 = 1024;

#[cfg(feature = "metavoice")]
enum Transformer {
    Normal(transformer::Model),
    Quantized(qtransformer::Model),
}

#[cfg(feature = "metavoice")]
pub async fn metavoice(prompt: String) -> Result<Bytes, Error> {
    use tracing_chrome::ChromeLayerBuilder;
    use tracing_subscriber::prelude::*;

    let show_status = false;
    let tracing = false;
    let cpu = false;
    let guidance_scale = 3.0;
    let temperature = 1.0;
    // Override seed for now
    let mut seed: Option<u64> = Some(299792458);
    let max_tokens = 2000;
    let first_stage_meta: Option<String> = None;
    let first_stage_weights: Option<String> = None;
    let second_stage_weights: Option<String> = None;
    let encodec_weights: Option<String> = None;
    let spk_emb: Option<String> = None;
    let dtype = DType::F32;
    let quantized = true;

    if seed.is_none() {
        seed = Some(rand::random());
    }

    let _guard = if tracing {
        let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
        tracing_subscriber::registry().with(chrome_layer).init();
        Some(guard)
    } else {
        None
    };

    let device = candle_examples::device(cpu)?;
    let api = Api::new()?;
    let repo = api.model("lmz/candle-metavoice".to_string());
    let first_stage_meta = match &first_stage_meta {
        Some(w) => std::path::PathBuf::from(w),
        None => repo.get("first_stage.meta.json")?,
    };
    let first_stage_meta: serde_json::Value =
        serde_json::from_reader(&std::fs::File::open(first_stage_meta)?)?;
    let first_stage_tokenizer = match first_stage_meta.as_object() {
        None => anyhow::bail!("not a json object"),
        Some(j) => match j.get("tokenizer") {
            None => anyhow::bail!("no tokenizer key"),
            Some(j) => j,
        },
    };
    let fs_tokenizer = tokenizers::BPE::from_json(first_stage_tokenizer, 512)?;

    let second_stage_weights = match &second_stage_weights {
        Some(w) => std::path::PathBuf::from(w),
        None => repo.get("second_stage.safetensors")?,
    };
    let encodec_weights = match encodec_weights {
        Some(w) => std::path::PathBuf::from(w),
        None => Api::new()?
            .model("facebook/encodec_24khz".to_string())
            .get("model.safetensors")?,
    };
    let first_stage_config = transformer::Config::cfg1b_v0_1();
    let mut first_stage_model = if quantized {
        let filename = match &first_stage_weights {
            Some(w) => std::path::PathBuf::from(w),
            None => repo.get("first_stage_q4k.gguf")?,
        };
        let vb =
            candle_transformers::quantized_var_builder::VarBuilder::from_gguf(filename, &device)?;
        let first_stage_model = qtransformer::Model::new(&first_stage_config, vb)?;
        Transformer::Quantized(first_stage_model)
    } else {
        let first_stage_weights = match &first_stage_weights {
            Some(w) => std::path::PathBuf::from(w),
            None => repo.get("first_stage.safetensors")?,
        };
        let first_stage_vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[first_stage_weights], dtype, &device)? };
        let first_stage_model = transformer::Model::new(&first_stage_config, first_stage_vb)?;
        Transformer::Normal(first_stage_model)
    };

    let second_stage_vb =
        unsafe { VarBuilder::from_mmaped_safetensors(&[second_stage_weights], dtype, &device)? };
    let second_stage_config = gpt::Config::cfg1b_v0_1();
    let second_stage_model = gpt::Model::new(second_stage_config.clone(), second_stage_vb)?;

    let encodec_device = if device.is_metal() {
        &candle_core::Device::Cpu
    } else {
        &device
    };
    let encodec_vb =
        unsafe { VarBuilder::from_mmaped_safetensors(&[encodec_weights], dtype, encodec_device)? };
    let encodec_config = encodec::Config::default();
    let encodec_model = encodec::Model::new(&encodec_config, encodec_vb)?;

    log::debug!("prompt: '{}'", prompt);
    let prompt_tokens = fs_tokenizer.encode(&prompt)?;
    let mut tokens = prompt_tokens.clone();
    log::debug!("{tokens:?}");
    let spk_emb_file = match &spk_emb {
        Some(w) => std::path::PathBuf::from(w),
        None => repo.get("spk_emb.safetensors")?,
    };
    let spk_emb = candle_core::safetensors::load(&spk_emb_file, &candle_core::Device::Cpu)?;
    let spk_emb = match spk_emb.get("spk_emb") {
        None => anyhow::bail!("missing spk_emb tensor in {spk_emb_file:?}"),
        Some(spk_emb) => spk_emb.to_dtype(dtype)?,
    };
    let spk_emb = spk_emb.to_device(&device)?;
    let seed_u64 = seed.unwrap_or_else(|| rand::thread_rng().gen());
    let mut logits_processor = LogitsProcessor::new(seed_u64, Some(temperature), Some(0.95));

    // First stage generation.
    for index in 0..max_tokens {
        let context_size = if index > 0 { 1 } else { tokens.len() };
        let start_pos = tokens.len().saturating_sub(context_size);
        let ctxt = &tokens[start_pos..];
        let input = Tensor::new(ctxt, &device)?;
        let input = Tensor::stack(&[&input, &input], 0)?;
        let logits = match &mut first_stage_model {
            Transformer::Normal(m) => m.forward(&input, &spk_emb, tokens.len() - context_size)?,
            Transformer::Quantized(m) => {
                m.forward(&input, &spk_emb, tokens.len() - context_size)?
            }
        };
        let logits0 = logits.i((0, 0))?;
        let logits1 = logits.i((1, 0))?;
        let logits = ((logits0 * guidance_scale)? + logits1 * (1. - guidance_scale))?;
        let logits = logits.to_dtype(DType::F32)?;
        let next_token = logits_processor.sample(&logits)?;
        tokens.push(next_token);
        if show_status {
            print!(".");
        }
        std::io::stdout().flush()?;
        if next_token == 2048 {
            break;
        }
    }
    if show_status {
        println!("");
    }
    let fie2c = adapters::FlattenedInterleavedEncodec2Codebook::new(ENCODEC_NTOKENS);
    let (text_ids, ids1, ids2) = fie2c.decode(&tokens);
    log::debug!("text ids len: {}", text_ids.len());
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed_u64 + 1337);
    // TODO: Use the config rather than hardcoding the offset here.
    let encoded_text: Vec<_> = prompt_tokens.iter().map(|v| v - 1024).collect();
    let mut hierarchies_in1 =
        [encoded_text.as_slice(), ids1.as_slice(), &[ENCODEC_NTOKENS]].concat();
    let mut hierarchies_in2 = [
        vec![ENCODEC_NTOKENS; encoded_text.len()].as_slice(),
        ids2.as_slice(),
        &[ENCODEC_NTOKENS],
    ]
    .concat();
    hierarchies_in1.resize(second_stage_config.block_size, ENCODEC_NTOKENS);
    hierarchies_in2.resize(second_stage_config.block_size, ENCODEC_NTOKENS);
    let in_x1 = Tensor::new(hierarchies_in1, &device)?;
    let in_x2 = Tensor::new(hierarchies_in2, &device)?;
    let in_x = Tensor::stack(&[in_x1, in_x2], 0)?.unsqueeze(0)?;
    let logits = second_stage_model.forward(&in_x)?;
    log::debug!("sampling from logits...");
    let mut codes = vec![];
    for logits in logits.iter() {
        let logits = logits.squeeze(0)?;
        let (seq_len, _) = logits.dims2()?;
        let mut codes_ = Vec::with_capacity(seq_len);
        for step in 0..seq_len {
            let logits = logits.i(step)?.to_dtype(DType::F32)?;
            let logits = &(&logits / 1.0)?;
            let prs = candle_nn::ops::softmax_last_dim(logits)?.to_vec1::<f32>()?;
            let distr = rand::distributions::WeightedIndex::new(prs.as_slice())?;
            let sample = distr.sample(&mut rng) as u32;
            codes_.push(sample)
        }
        codes.push(codes_)
    }

    let codes = Tensor::new(codes, &device)?.unsqueeze(0)?;
    let codes = Tensor::cat(&[in_x, codes], 1)?;
    log::debug!("codes: {codes}");
    let tilted_encodec = adapters::TiltedEncodec::new(ENCODEC_NTOKENS);
    let codes = codes.i(0)?.to_vec2::<u32>()?;
    let (text_ids, audio_ids) = tilted_encodec.decode(&codes);
    log::debug!("text_ids len: {:?}", text_ids.len());
    let audio_ids = Tensor::new(audio_ids, encodec_device)?.unsqueeze(0)?;
    log::debug!("audio_ids shape: {:?}", audio_ids.shape());
    let pcm = encodec_model.decode(&audio_ids)?;
    log::debug!("output pcm shape: {:?}", pcm.shape());
    let pcm = pcm.i(0)?.i(0)?.to_dtype(DType::F32)?;
    let pcm = candle_examples::audio::normalize_loudness(&pcm, 24_000, true)?;
    let pcm = pcm.to_vec1::<f32>()?;

    // Create a buffer to hold the WAV data
    let mut buffer = Cursor::new(Vec::new());

    // Write the PCM data to the buffer as a WAV file
    candle_examples::wav::write_pcm_as_wav(&mut buffer, &pcm, 24_000)?;

    // Get the buffer's content as Bytes
    let bytes = buffer.into_inner().into();

    // Return the Bytes containing the WAV data
    Ok(bytes)
}
