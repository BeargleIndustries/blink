use serde::Serialize;

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct AppError {
    pub message: String,
    pub recovery: Option<String>,
}

impl From<sd_wrapper::SdError> for AppError {
    fn from(err: sd_wrapper::SdError) -> Self {
        let (message, recovery) = match &err {
            sd_wrapper::SdError::ContextCreationFailed { .. } => (
                err.to_string(),
                Some("Try re-downloading the model.".into()),
            ),
            sd_wrapper::SdError::InferenceReturnedNull => (
                "Image generation failed unexpectedly.".into(),
                Some("Try different parameters or a different model.".into()),
            ),
            sd_wrapper::SdError::ModelNotFound { .. } => (
                err.to_string(),
                Some("Re-download the model from the Model Browser.".into()),
            ),
            sd_wrapper::SdError::ModelHashMismatch { .. } => (
                "Model file appears corrupted (checksum mismatch).".into(),
                Some("Delete and re-download the model.".into()),
            ),
            sd_wrapper::SdError::DownloadFailed { .. } => (
                err.to_string(),
                Some("Check your internet connection and try again.".into()),
            ),
            sd_wrapper::SdError::DownloadInterrupted => (
                "Download interrupted.".into(),
                Some("Click Resume to continue.".into()),
            ),
            sd_wrapper::SdError::InsufficientDiskSpace { .. } => (
                err.to_string(),
                Some("Free up disk space or choose a smaller model.".into()),
            ),
            sd_wrapper::SdError::GpuBackendUnavailable { .. } => (
                err.to_string(),
                None, // Informational only
            ),
            sd_wrapper::SdError::OutOfVram { .. } => (
                err.to_string(),
                Some("Try a smaller/more quantized model, or use CPU mode.".into()),
            ),
            sd_wrapper::SdError::Cancelled => (
                "Generation cancelled.".into(),
                None,
            ),
            _ => (err.to_string(), None),
        };
        AppError { message, recovery }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
