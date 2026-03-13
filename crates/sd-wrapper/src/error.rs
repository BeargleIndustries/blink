use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdError {
    #[error("Failed to create model context: {reason}")]
    ContextCreationFailed { reason: String },

    #[error("Inference returned null — model may be corrupted or incompatible")]
    InferenceReturnedNull,

    #[error("FFI panic caught in inference thread: {message}")]
    FfiPanic { message: String },

    #[error("Model file not found: {path}")]
    ModelNotFound { path: String },

    #[error("Model file corrupted — SHA256 mismatch (expected {expected}, got {actual})")]
    ModelHashMismatch { expected: String, actual: String },

    #[error("Unsupported model format: {path}")]
    UnsupportedModelFormat { path: String },

    #[error("Download failed: {url} — {reason}")]
    DownloadFailed { url: String, reason: String },

    #[error("Download interrupted — can be resumed")]
    DownloadInterrupted,

    #[error("Insufficient disk space: need {needed_mb}MB, have {available_mb}MB")]
    InsufficientDiskSpace { needed_mb: u64, available_mb: u64 },

    #[error("GPU backend unavailable: {backend} — falling back to CPU")]
    GpuBackendUnavailable { backend: String },

    #[error("Out of VRAM: model needs ~{needed_mb}MB, GPU has {available_mb}MB")]
    OutOfVram { needed_mb: u64, available_mb: u64 },

    #[error("Generation cancelled by user")]
    Cancelled,

    #[error("Invalid parameters: {reason}")]
    InvalidParams { reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_not_found_error_includes_path() {
        let err = SdError::ModelNotFound {
            path: "/tmp/missing.gguf".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("/tmp/missing.gguf"));
    }

    #[test]
    fn cancelled_error_is_descriptive() {
        let err = SdError::Cancelled;
        let msg = format!("{}", err);
        assert!(msg.contains("cancelled"));
    }
}
