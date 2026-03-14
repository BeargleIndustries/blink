use std::env;
use std::path::PathBuf;

fn main() {
    // Register custom cfg names to suppress unexpected_cfgs warnings
    println!("cargo::rustc-check-cfg=cfg(has_cuda)");
    println!("cargo::rustc-check-cfg=cfg(has_metal)");
    println!("cargo::rustc-check-cfg=cfg(has_vulkan)");

    // Auto-detect GPU backends (mirrors sd-sys detection logic).
    // cfg flags emitted here apply to sd-wrapper and its dependents.
    if let Some(reason) = detect_cuda() {
        println!("cargo:rustc-cfg=has_cuda");
        let _ = reason; // already warned from sd-sys
    }
    if detect_metal() {
        println!("cargo:rustc-cfg=has_metal");
    }
    if let Some(reason) = detect_vulkan() {
        println!("cargo:rustc-cfg=has_vulkan");
        let _ = reason;
    }

    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
}

fn detect_cuda() -> Option<String> {
    if cfg!(feature = "cuda") {
        return Some("feature flag".into());
    }
    if let Ok(cuda_path) = env::var("CUDA_PATH") {
        if PathBuf::from(&cuda_path).exists() {
            return Some(format!("CUDA_PATH={}", cuda_path));
        }
    }
    if std::process::Command::new("nvcc")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("nvcc in PATH".into());
    }
    if cfg!(target_os = "windows") {
        let base = PathBuf::from(r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA");
        if base.exists() {
            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        return Some(format!("found at {}", entry.path().display()));
                    }
                }
            }
        }
    }
    if cfg!(target_os = "linux") {
        let cuda_path = PathBuf::from("/usr/local/cuda");
        if cuda_path.exists() {
            return Some("found at /usr/local/cuda".into());
        }
    }
    None
}

fn detect_metal() -> bool {
    if cfg!(feature = "metal") {
        return true;
    }
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    target_os == "macos"
}

fn detect_vulkan() -> Option<String> {
    if cfg!(feature = "vulkan") {
        return Some("feature flag".into());
    }
    if let Ok(sdk_path) = env::var("VULKAN_SDK") {
        if PathBuf::from(&sdk_path).exists() {
            return Some(format!("VULKAN_SDK={}", sdk_path));
        }
    }
    None
}
