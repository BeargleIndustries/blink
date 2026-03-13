#[derive(Debug, Clone)]
pub struct ContextConfig {
    pub model_path: String,
    pub vae_path: Option<String>,
    pub n_threads: i32,
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
