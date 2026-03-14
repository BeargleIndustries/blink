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
    // Performance options
    pub flash_attn: bool,
    pub diffusion_flash_attn: bool,
    pub enable_mmap: bool,
    pub free_params_immediately: bool,
    // VRAM management — offload stages to CPU to save GPU memory
    pub keep_clip_on_cpu: bool,
    pub keep_vae_on_cpu: bool,
    pub offload_params_to_cpu: bool,
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
    fn default_img2img_params_have_valid_strength() {
        let params = Img2ImgParams::default();
        assert!(params.strength > 0.0 && params.strength <= 1.0);
    }
}
