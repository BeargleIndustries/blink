//! GPU backend detection and fallback logic.

use std::fmt;

/// Active GPU backend for inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GpuBackend {
    Cpu,
    Metal,
    Cuda,
    Vulkan,
}

impl fmt::Display for GpuBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GpuBackend::Cpu => write!(f, "CPU"),
            GpuBackend::Metal => write!(f, "Metal"),
            GpuBackend::Cuda => write!(f, "CUDA"),
            GpuBackend::Vulkan => write!(f, "Vulkan"),
        }
    }
}

/// Detect the compiled GPU backend based on cargo feature flags.
/// This returns what was compiled in, not necessarily what's available at runtime.
/// sd.cpp handles runtime fallback internally.
pub fn compiled_backend() -> GpuBackend {
    #[cfg(feature = "metal")]
    return GpuBackend::Metal;

    #[cfg(feature = "cuda")]
    return GpuBackend::Cuda;

    #[cfg(feature = "vulkan")]
    return GpuBackend::Vulkan;

    #[cfg(not(any(feature = "metal", feature = "cuda", feature = "vulkan")))]
    return GpuBackend::Cpu;
}
