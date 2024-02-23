#[cfg(feature = "accelerate")]
extern crate accelerate_src;

#[cfg(feature = "mkl")]
extern crate intel_mkl_src;

use candle_transformers::models::stable_diffusion;

use anyhow::{Error as E, Result};
use candle_core::{DType, Device, IndexOp, Module, Tensor, D};
use image::ImageBuffer;
use log::info;
use tokenizers::Tokenizer;

#[derive(Debug, Clone, Copy, clap::ValueEnum, PartialEq, Eq)]
pub enum StableDiffusionVersion {
    V1_5,
    V2_1,
    Xl,
    Turbo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelFile {
    Tokenizer,
    Tokenizer2,
    Clip,
    Clip2,
    Unet,
    Vae,
}

impl StableDiffusionVersion {
    fn repo(&self) -> &'static str {
        match self {
            Self::Xl => "stabilityai/stable-diffusion-xl-base-1.0",
            Self::V2_1 => "stabilityai/stable-diffusion-2-1",
            Self::V1_5 => "runwayml/stable-diffusion-v1-5",
            Self::Turbo => "stabilityai/sdxl-turbo",
        }
    }

    fn unet_file(&self, use_f16: bool) -> &'static str {
        match self {
            Self::V1_5 | Self::V2_1 | Self::Xl | Self::Turbo => {
                if use_f16 {
                    "unet/diffusion_pytorch_model.fp16.safetensors"
                } else {
                    "unet/diffusion_pytorch_model.safetensors"
                }
            }
        }
    }

    fn vae_file(&self, use_f16: bool) -> &'static str {
        match self {
            Self::V1_5 | Self::V2_1 | Self::Xl | Self::Turbo => {
                if use_f16 {
                    "vae/diffusion_pytorch_model.fp16.safetensors"
                } else {
                    "vae/diffusion_pytorch_model.safetensors"
                }
            }
        }
    }

    fn clip_file(&self, use_f16: bool) -> &'static str {
        match self {
            Self::V1_5 | Self::V2_1 | Self::Xl | Self::Turbo => {
                if use_f16 {
                    "text_encoder/model.fp16.safetensors"
                } else {
                    "text_encoder/model.safetensors"
                }
            }
        }
    }

    fn clip2_file(&self, use_f16: bool) -> &'static str {
        match self {
            Self::V1_5 | Self::V2_1 | Self::Xl | Self::Turbo => {
                if use_f16 {
                    "text_encoder_2/model.fp16.safetensors"
                } else {
                    "text_encoder_2/model.safetensors"
                }
            }
        }
    }
}

impl ModelFile {
    fn get(
        &self,
        filename: Option<String>,
        version: StableDiffusionVersion,
        use_f16: bool,
    ) -> Result<std::path::PathBuf> {
        use hf_hub::api::sync::Api;
        match filename {
            Some(filename) => Ok(std::path::PathBuf::from(filename)),
            None => {
                let (repo, path) = match self {
                    Self::Tokenizer => {
                        let tokenizer_repo = match version {
                            StableDiffusionVersion::V1_5 | StableDiffusionVersion::V2_1 => {
                                "openai/clip-vit-base-patch32"
                            }
                            StableDiffusionVersion::Xl | StableDiffusionVersion::Turbo => {
                                // This seems similar to the patch32 version except some very small
                                // difference in the split regex.
                                "openai/clip-vit-large-patch14"
                            }
                        };
                        (tokenizer_repo, "tokenizer.json")
                    }
                    Self::Tokenizer2 => {
                        ("laion/CLIP-ViT-bigG-14-laion2B-39B-b160k", "tokenizer.json")
                    }
                    Self::Clip => (version.repo(), version.clip_file(use_f16)),
                    Self::Clip2 => (version.repo(), version.clip2_file(use_f16)),
                    Self::Unet => (version.repo(), version.unet_file(use_f16)),
                    Self::Vae => {
                        // Override for SDXL when using f16 weights.
                        // See https://github.com/huggingface/candle/issues/1060
                        if matches!(
                            version,
                            StableDiffusionVersion::Xl | StableDiffusionVersion::Turbo,
                        ) && use_f16
                        {
                            (
                                "madebyollin/sdxl-vae-fp16-fix",
                                "diffusion_pytorch_model.safetensors",
                            )
                        } else {
                            (version.repo(), version.vae_file(use_f16))
                        }
                    }
                };
                let filename = Api::new()?.model(repo.to_string()).get(path)?;
                Ok(filename)
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn text_embeddings(
    prompt: &str,
    uncond_prompt: &str,
    tokenizer: Option<String>,
    clip_weights: Option<String>,
    sd_version: StableDiffusionVersion,
    sd_config: &stable_diffusion::StableDiffusionConfig,
    use_f16: bool,
    device: &Device,
    dtype: DType,
    use_guide_scale: bool,
    first: bool,
) -> Result<Tensor> {
    let tokenizer_file = if first {
        ModelFile::Tokenizer
    } else {
        ModelFile::Tokenizer2
    };
    let tokenizer = tokenizer_file.get(tokenizer, sd_version, use_f16)?;
    let tokenizer = Tokenizer::from_file(tokenizer).map_err(E::msg)?;
    let pad_id = match &sd_config.clip.pad_with {
        Some(padding) => *tokenizer.get_vocab(true).get(padding.as_str()).unwrap(),
        None => *tokenizer.get_vocab(true).get("<|endoftext|>").unwrap(),
    };
    info!("Stable Diffusion: Running with prompt \"{prompt}\".");
    let mut tokens = tokenizer
        .encode(prompt, true)
        .map_err(E::msg)?
        .get_ids()
        .to_vec();
    while tokens.len() < sd_config.clip.max_position_embeddings {
        tokens.push(pad_id)
    }
    let tokens = Tensor::new(tokens.as_slice(), device)?.unsqueeze(0)?;

    info!("Stable Diffusion: Building the Clip transformer.");
    let clip_weights_file = if first {
        ModelFile::Clip
    } else {
        ModelFile::Clip2
    };
    let clip_weights = clip_weights_file.get(clip_weights, sd_version, false)?;
    let clip_config = if first {
        &sd_config.clip
    } else {
        sd_config.clip2.as_ref().unwrap()
    };
    let text_model =
        stable_diffusion::build_clip_transformer(clip_config, clip_weights, device, DType::F32)?;
    let text_embeddings = text_model.forward(&tokens)?;

    let text_embeddings = if use_guide_scale {
        let mut uncond_tokens = tokenizer
            .encode(uncond_prompt, true)
            .map_err(E::msg)?
            .get_ids()
            .to_vec();
        while uncond_tokens.len() < sd_config.clip.max_position_embeddings {
            uncond_tokens.push(pad_id)
        }

        let uncond_tokens = Tensor::new(uncond_tokens.as_slice(), device)?.unsqueeze(0)?;
        let uncond_embeddings = text_model.forward(&uncond_tokens)?;

        Tensor::cat(&[uncond_embeddings, text_embeddings], 0)?.to_dtype(dtype)?
    } else {
        text_embeddings.to_dtype(dtype)?
    };
    Ok(text_embeddings)
}

fn image_preprocess<T: AsRef<std::path::Path>>(path: T) -> anyhow::Result<Tensor> {
    let img = image::io::Reader::open(path)?.decode()?;
    let (height, width) = (img.height() as usize, img.width() as usize);
    let height = height - height % 32;
    let width = width - width % 32;
    let img = img.resize_to_fill(
        width as u32,
        height as u32,
        image::imageops::FilterType::CatmullRom,
    );
    let img = img.to_rgb8();
    let img = img.into_raw();
    let img = Tensor::from_vec(img, (height, width, 3), &Device::Cpu)?
        .permute((2, 0, 1))?
        .to_dtype(DType::F32)?
        .affine(2. / 255., -1.)?
        .unsqueeze(0)?;
    Ok(img)
}

pub struct SDConfig {
    pub prompt: String,
    pub uncond_prompt: String,
    pub cpu: bool,
    pub tracing: bool,
    pub height: Option<usize>,
    pub width: Option<usize>,
    pub unet_weights: Option<String>,
    pub clip_weights: Option<String>,
    pub vae_weights: Option<String>,
    pub tokenizer: Option<String>,
    pub sliced_attention_size: Option<usize>,
    pub n_steps: Option<usize>,
    pub num_samples: usize,
    pub sd_version: StableDiffusionVersion,
    pub intermediary_images: bool,
    pub use_flash_attn: bool,
    pub use_f16: bool,
    pub guidance_scale: Option<f64>,
    pub img2img: Option<String>,
    pub img2img_strength: f64,
}

impl SDConfig {
    // Providing a method to create a new SDConfig with default values
    pub fn new() -> Self {
        SDConfig {
            prompt: "A very realistic photo of a rusty robot walking on a sandy beach".into(),
            uncond_prompt: "".into(),
            cpu: false,
            tracing: false,
            height: Some(512),
            width: Some(512),
            unet_weights: None,
            clip_weights: None,
            vae_weights: None,
            tokenizer: None,
            sliced_attention_size: None,
            n_steps: None,
            num_samples: 1,
            sd_version: StableDiffusionVersion::Turbo,
            intermediary_images: true,
            use_flash_attn: false,
            use_f16: false,
            guidance_scale: None,
            img2img: None,
            img2img_strength: 0.8,
        }
    }
}

pub fn sd(config: SDConfig) -> Result<Vec<ImageBuffer<image::Rgb<u8>, Vec<u8>>>> {
    use tracing_chrome::ChromeLayerBuilder;
    use tracing_subscriber::prelude::*;

    if !(0. ..=1.).contains(&config.img2img_strength) {
        anyhow::bail!(
            "img2img-strength should be between 0 and 1, got {0}",
            config.img2img_strength
        )
    }

    let _guard = if config.tracing {
        let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
        tracing_subscriber::registry().with(chrome_layer).init();
        Some(guard)
    } else {
        None
    };

    let guidance_scale = match config.guidance_scale {
        Some(guidance_scale) => guidance_scale,
        None => match config.sd_version {
            StableDiffusionVersion::V1_5
            | StableDiffusionVersion::V2_1
            | StableDiffusionVersion::Xl => 7.5,
            StableDiffusionVersion::Turbo => 0.,
        },
    };
    let n_steps = match config.n_steps {
        Some(n_steps) => n_steps,
        None => match config.sd_version {
            StableDiffusionVersion::V1_5
            | StableDiffusionVersion::V2_1
            | StableDiffusionVersion::Xl => 30,
            StableDiffusionVersion::Turbo => 1,
        },
    };
    let dtype = if config.use_f16 {
        DType::F16
    } else {
        DType::F32
    };
    let sd_config = match config.sd_version {
        StableDiffusionVersion::V1_5 => stable_diffusion::StableDiffusionConfig::v1_5(
            config.sliced_attention_size,
            config.height,
            config.width,
        ),
        StableDiffusionVersion::V2_1 => stable_diffusion::StableDiffusionConfig::v2_1(
            config.sliced_attention_size,
            config.height,
            config.width,
        ),
        StableDiffusionVersion::Xl => stable_diffusion::StableDiffusionConfig::sdxl(
            config.sliced_attention_size,
            config.height,
            config.width,
        ),
        StableDiffusionVersion::Turbo => stable_diffusion::StableDiffusionConfig::sdxl_turbo(
            config.sliced_attention_size,
            config.height,
            config.width,
        ),
    };

    let scheduler = sd_config.build_scheduler(n_steps)?;
    let device = candle_examples::device(config.cpu)?;
    let use_guide_scale = guidance_scale > 1.0;

    let which = match config.sd_version {
        StableDiffusionVersion::Xl | StableDiffusionVersion::Turbo => vec![true, false],
        _ => vec![true],
    };

    let text_embeddings = which
        .iter()
        .map(|first| {
            text_embeddings(
                &config.prompt,
                &config.uncond_prompt,
                config.tokenizer.clone(),
                config.clip_weights.clone(),
                config.sd_version,
                &sd_config,
                config.use_f16,
                &device,
                dtype,
                use_guide_scale,
                *first,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let text_embeddings = Tensor::cat(&text_embeddings, D::Minus1)?;
    info!("Stable Diffusion: Text Embeddings - {text_embeddings:?}");

    info!("Stable Diffusion: Building the autoencoder.");
    let vae_weights = ModelFile::Vae.get(config.vae_weights, config.sd_version, config.use_f16)?;
    let vae = sd_config.build_vae(vae_weights, &device, dtype)?;
    let init_latent_dist = match &config.img2img {
        None => None,
        Some(image_buf) => {
            let image_buf = image_preprocess(image_buf)?.to_device(&device)?;
            Some(vae.encode(&image_buf)?)
        }
    };
    info!("Stable Diffusion: Building the unet.");
    let unet_weights =
        ModelFile::Unet.get(config.unet_weights, config.sd_version, config.use_f16)?;
    let unet = sd_config.build_unet(unet_weights, &device, 4, config.use_flash_attn, dtype)?;

    let t_start = if config.img2img.is_some() {
        n_steps - (n_steps as f64 * config.img2img_strength) as usize
    } else {
        0
    };
    let bsize = 1;

    let vae_scale = match config.sd_version {
        StableDiffusionVersion::V1_5
        | StableDiffusionVersion::V2_1
        | StableDiffusionVersion::Xl => 0.18215,
        StableDiffusionVersion::Turbo => 0.13025,
    };

    // array of image buffers to gather the results
    let mut images = Vec::with_capacity(config.num_samples);

    for idx in 0..config.num_samples {
        let timesteps = scheduler.timesteps();
        let latents = match &init_latent_dist {
            Some(init_latent_dist) => {
                let latents = (init_latent_dist.sample()? * vae_scale)?.to_device(&device)?;
                if t_start < timesteps.len() {
                    let noise = latents.randn_like(0f64, 1f64)?;
                    scheduler.add_noise(&latents, noise, timesteps[t_start])?
                } else {
                    latents
                }
            }
            None => {
                let latents = Tensor::randn(
                    0f32,
                    1f32,
                    (bsize, 4, sd_config.height / 8, sd_config.width / 8),
                    &device,
                )?;
                // scale the initial noise by the standard deviation required by the scheduler
                (latents * scheduler.init_noise_sigma())?
            }
        };
        let mut latents = latents.to_dtype(dtype)?;

        info!("Stable Diffusion: starting sampling");
        for (timestep_index, &timestep) in timesteps.iter().enumerate() {
            if timestep_index < t_start {
                continue;
            }
            let start_time = std::time::Instant::now();
            let latent_model_input = if use_guide_scale {
                Tensor::cat(&[&latents, &latents], 0)?
            } else {
                latents.clone()
            };

            let latent_model_input = scheduler.scale_model_input(latent_model_input, timestep)?;
            let noise_pred =
                unet.forward(&latent_model_input, timestep as f64, &text_embeddings)?;

            let noise_pred = if use_guide_scale {
                let noise_pred = noise_pred.chunk(2, 0)?;
                let (noise_pred_uncond, noise_pred_text) = (&noise_pred[0], &noise_pred[1]);

                (noise_pred_uncond + ((noise_pred_text - noise_pred_uncond)? * guidance_scale)?)?
            } else {
                noise_pred
            };

            latents = scheduler.step(&noise_pred, timestep, &latents)?;
            let dt = start_time.elapsed().as_secs_f32();
            info!(
                "Stable Diffusion: step {}/{n_steps} done, {:.2}s",
                timestep_index + 1,
                dt
            );

            if config.intermediary_images {
                let image_buf = vae.decode(&(&latents / vae_scale)?)?;
                let image_buf = ((image_buf / 2.)? + 0.5)?.to_device(&Device::Cpu)?;
                let image_buf = (image_buf * 255.)?.to_dtype(DType::U8)?.i(0)?;
                let (channel, height, width) = image_buf.dims3()?;
                if channel != 3 {
                    anyhow::bail!("save_image expects an input of shape (3, height, width)")
                }
                let img = image_buf.permute((1, 2, 0))?.flatten_all()?;
                let pixels = img.to_vec1::<u8>()?;
                let image_u8: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
                    match image::ImageBuffer::from_raw(width as u32, height as u32, pixels) {
                        Some(image_u8) => image_u8,
                        None => anyhow::bail!("error saving image"),
                    };

                images.push(image_u8);
            }
        }

        info!(
            "Stable Diffusion: Generating the final image for sample {}/{}.",
            idx + 1,
            config.num_samples
        );
        let image_buf = vae.decode(&(&latents / vae_scale)?)?;
        let image_buf = ((image_buf / 2.)? + 0.5)?.to_device(&Device::Cpu)?;
        let image_buf = (image_buf.clamp(0f32, 1.)? * 255.)?
            .to_dtype(DType::U8)?
            .i(0)?;

        let (channel, height, width) = image_buf.dims3()?;
        if channel != 3 {
            anyhow::bail!("save_image expects an input of shape (3, height, width)")
        }
        let img = image_buf.permute((1, 2, 0))?.flatten_all()?;
        let pixels = img.to_vec1::<u8>()?;
        let image_u8: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            match image::ImageBuffer::from_raw(width as u32, height as u32, pixels) {
                Some(image_u8) => image_u8,
                None => anyhow::bail!("error saving image"),
            };

        images.push(image_u8);
    }

    Ok(images)
}
