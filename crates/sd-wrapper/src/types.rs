/// How sd.cpp should apply LoRA weights.
///
/// `Auto` mirrors sd.cpp's own choice: it falls back to `AtRuntime` whenever the
/// model has quantized weights, parameters are offloaded, or layer streaming is
/// active — all of which are true for Blink's quantized Z-Image setup. The runtime
/// path re-reads the LoRA file once per backend module, which is the behaviour
/// reported in sd.cpp issue #1071.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoraApplyMode {
    Auto,
    /// Merge LoRA into the weights up front. Faster inference, but sd.cpp warns it
    /// can lose precision or fail outright against quantized parameters.
    Immediately,
    /// Apply during the forward pass. Better compatibility and precision.
    AtRuntime,
}

#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub model_path: Option<String>,
    pub vae_path: Option<String>,
    // Flux-specific model paths
    pub clip_l_path: Option<String>,
    pub t5xxl_path: Option<String>,
    pub diffusion_model_path: Option<String>,
    // Z-Image / LLM-encoder models
    pub llm_path: Option<String>,
    pub n_threads: i32,
    /// Let sd.cpp place diffusion/TE/VAE from measured VRAM. Mutually exclusive
    /// with an explicit params_backend — sd.cpp ignores the latter when this is set.
    pub auto_fit: bool,
    /// Escape hatch: force every parameter to CPU RAM (Blink's pre-auto_fit
    /// behaviour). Overrides auto_fit.
    pub low_memory: bool,
    // ControlNet model path
    pub control_net_path: Option<String>,
    // TAESD model path for live previews
    pub taesd_path: Option<String>,
    /// LoRA application strategy. Defaults to `Auto` (sd.cpp's own choice).
    pub lora_apply_mode: LoraApplyMode,
}

impl ContextConfig {
    /// Whether sd.cpp should be asked to auto-fit parameter placement.
    ///
    /// `low_memory` wins: sd.cpp's `derive_backend_specs()` overwrites
    /// `params_backend` whenever `auto_fit` is set, so the two must never be
    /// requested together.
    pub fn uses_auto_fit(&self) -> bool {
        !self.low_memory && self.auto_fit
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            model_path: None,
            vae_path: None,
            clip_l_path: None,
            t5xxl_path: None,
            diffusion_model_path: None,
            llm_path: None,
            n_threads: 4,
            auto_fit: true,
            low_memory: false,
            control_net_path: None,
            taesd_path: None,
            lora_apply_mode: LoraApplyMode::Auto,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoraConfig {
    pub path: String,
    pub multiplier: f32,
    pub is_high_noise: bool,
}

#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub prompt: String,
    pub negative_prompt: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub cfg_scale: f32,
    pub seed: i64,
    pub sample_method: SampleMethod,
    pub batch_count: u32,
    // Kontext ref images for image editing
    pub ref_images: Vec<Vec<u8>>,
    // Image conditioning strength (Kontext)
    pub img_cfg: Option<f32>,
    // LoRA adapters
    pub loras: Vec<LoraConfig>,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            negative_prompt: String::new(),
            width: 512,
            height: 512,
            steps: 20,
            cfg_scale: 7.0,
            seed: -1,
            sample_method: SampleMethod::EulerA,
            batch_count: 1,
            ref_images: Vec::new(),
            img_cfg: None,
            loras: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Img2ImgParams {
    pub base: GenerationParams,
    pub strength: f32,
}

impl Default for Img2ImgParams {
    fn default() -> Self {
        Self {
            base: GenerationParams::default(),
            strength: 0.75,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum SampleMethod {
    Euler,
    EulerA,
    Heun,
    Dpm2,
    DpmPlusPlus2m,
    Lcm,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_generation_params_are_valid() {
        let params = GenerationParams::default();
        assert_eq!(params.width, 512);
        assert_eq!(params.height, 512);
        assert_eq!(params.steps, 20);
        assert!(params.cfg_scale > 0.0);
    }

    #[test]
    fn context_config_defaults_to_auto_fit() {
        let config = ContextConfig::default();
        assert!(config.auto_fit);
        assert!(!config.low_memory);
        assert!(config.uses_auto_fit());
    }

    #[test]
    fn low_memory_overrides_auto_fit() {
        let config = ContextConfig {
            low_memory: true,
            ..ContextConfig::default()
        };
        // auto_fit is still true, but low_memory wins — sd.cpp must be given
        // params_backend=cpu and never auto_fit, since auto_fit would overwrite it.
        assert!(config.auto_fit);
        assert!(!config.uses_auto_fit());
    }

    #[test]
    fn default_img2img_params_have_valid_strength() {
        let params = Img2ImgParams::default();
        assert!(params.strength > 0.0 && params.strength <= 1.0);
    }
}
