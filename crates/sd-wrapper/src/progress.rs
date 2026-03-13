#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub step: u32,
    pub total_steps: u32,
    pub elapsed_secs: f32,
    pub preview: Option<Vec<u8>>,
}

pub type ProgressCallback = Box<dyn Fn(ProgressUpdate) + Send>;
