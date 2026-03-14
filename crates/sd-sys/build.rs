use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sd_cpp_dir = manifest_dir.join("stable-diffusion.cpp");

    // --- Auto-detect GPU backends (feature flags serve as force-overrides) ---
    let use_cuda = detect_cuda();
    let use_metal = detect_metal();
    let use_vulkan = detect_vulkan();

    // Emit cfg flags for downstream crates (within sd-sys)
    if let Some(ref reason) = use_cuda {
        println!("cargo:rustc-cfg=has_cuda");
        println!("cargo:warning=GPU backend auto-detected: CUDA (via {})", reason);
    }
    if use_metal {
        println!("cargo:rustc-cfg=has_metal");
        println!("cargo:warning=GPU backend auto-detected: Metal (macOS)");
    }
    if let Some(ref reason) = use_vulkan {
        println!("cargo:rustc-cfg=has_vulkan");
        println!("cargo:warning=GPU backend auto-detected: Vulkan (via {})", reason);
    }
    if use_cuda.is_none() && !use_metal && use_vulkan.is_none() {
        println!("cargo:warning=No GPU backend detected — will build CPU-only (expect slow inference)");
    }

    // --- CMake build ---
    let mut cmake_cfg = cmake::Config::new(&sd_cpp_dir);

    cmake_cfg
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("SD_BUILD_SHARED_LIB", "OFF")
        .define("SD_BUILD_EXAMPLES", "OFF");

    if use_metal {
        cmake_cfg.define("SD_METAL", "ON");
    }
    if use_cuda.is_some() {
        cmake_cfg.define("SD_CUDA", "ON");
    }
    if use_vulkan.is_some() {
        cmake_cfg.define("SD_VULKAN", "ON");
    }

    // Read target info early (used for CMake config and linking)
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // macOS: std::filesystem requires 10.15+
    if target_os == "macos" {
        cmake_cfg.define("CMAKE_OSX_DEPLOYMENT_TARGET", "10.15");
    }

    // On Windows with CUDA, prefer Ninja generator over VS generators.
    // VS generators require the full VS CUDA integration which is fragile;
    // Ninja works directly with nvcc + MSVC cl.exe.
    if target_os == "windows" && use_cuda.is_some() {
        if std::process::Command::new("ninja").arg("--version").output().is_ok() {
            cmake_cfg.generator("Ninja");
        }
    }

    // Force Release build for the C++ libraries to avoid CRT mismatch.
    // Rust always links against the release CRT (msvcrt), so Debug-built C++
    // objects that reference debug CRT symbols (_CrtDbgReport, _malloc_dbg)
    // cause unresolved externals.
    if target_os == "windows" {
        cmake_cfg.profile("Release");
    }

    let dst = cmake_cfg.build();

    // --- Link search paths ---
    // CMake may place libraries in lib/, lib64/, or build/ depending on platform
    let lib_dir = dst.join("lib");
    let lib64_dir = dst.join("lib64");
    let build_dir = dst.join("build");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    if lib64_dir.exists() {
        println!("cargo:rustc-link-search=native={}", lib64_dir.display());
    }
    if build_dir.exists() {
        println!("cargo:rustc-link-search=native={}", build_dir.display());
    }

    // --- Discover and link all static libraries ---
    link_all_static_libs(&lib_dir);
    if lib64_dir.exists() {
        link_all_static_libs(&lib64_dir);
    }

    // --- System libraries ---
    match target_os.as_str() {
        "linux" => {
            println!("cargo:rustc-link-lib=stdc++");
            println!("cargo:rustc-link-lib=gomp"); // OpenMP (used by ggml-cpu)
        }
        "macos" => {
            println!("cargo:rustc-link-lib=c++");
            println!("cargo:rustc-link-lib=framework=Accelerate"); // vDSP (used by ggml)
            if use_metal {
                println!("cargo:rustc-link-lib=framework=Metal");
                println!("cargo:rustc-link-lib=framework=Foundation");
                println!("cargo:rustc-link-lib=framework=MetalPerformanceShaders");
            }
        }
        "windows" => {
            // MSVC links C++ runtime automatically.
            // For GNU toolchain on Windows, link stdc++.
            if target_env == "gnu" {
                println!("cargo:rustc-link-lib=stdc++");
            }
        }
        _ => {}
    }

    // --- CUDA system libraries ---
    if use_cuda.is_some() {
        // Add CUDA library search path
        if let Ok(cuda_path) = env::var("CUDA_PATH") {
            let cuda_lib = PathBuf::from(&cuda_path).join("lib").join("x64");
            if cuda_lib.exists() {
                println!("cargo:rustc-link-search=native={}", cuda_lib.display());
            }
            // Some installs use lib64 instead
            let cuda_lib64 = PathBuf::from(&cuda_path).join("lib64");
            if cuda_lib64.exists() {
                println!("cargo:rustc-link-search=native={}", cuda_lib64.display());
            }
        }
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rustc-link-lib=cudart");
        println!("cargo:rustc-link-lib=cublas");
        println!("cargo:rustc-link-lib=cublasLt");
    }

    // --- Vulkan system libraries ---
    if use_vulkan.is_some() {
        match target_os.as_str() {
            "windows" => {
                println!("cargo:rustc-link-lib=vulkan-1");
            }
            "linux" => {
                println!("cargo:rustc-link-lib=vulkan");
            }
            _ => {}
        }
    }

    // --- Bindgen ---
    let include_dir = sd_cpp_dir.join("include");
    let header_path = include_dir.join("stable-diffusion.h");

    let mut builder = bindgen::Builder::default()
        .header(header_path.to_str().unwrap())
        .clang_arg(format!("-I{}", include_dir.display()))
        // Force C enums to generate as top-level constants (type alias + pub const)
        .default_enum_style(bindgen::EnumVariation::Consts)
        .constified_enum(".*");

    // On MSVC, bindgen's bundled clang often can't find system headers (stdbool.h, etc.).
    // Use the cc crate to discover the MSVC toolchain include paths and pass them to clang.
    if let Ok(tool) = cc::Build::new().try_get_compiler() {
        for (key, value) in tool.env() {
            if key == "INCLUDE" {
                for path in std::env::split_paths(&value) {
                    if path.exists() {
                        builder = builder.clang_arg("-isystem").clang_arg(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    let bindings = builder
        .generate()
        .expect("Failed to generate bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Failed to write bindings");

    // --- Rerun triggers ---
    println!("cargo:rerun-if-changed={}", header_path.display());
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
}

/// Returns Some(reason) if CUDA should be enabled, None otherwise.
/// Feature flag acts as a force-override.
fn detect_cuda() -> Option<String> {
    // Feature flag force-override
    if cfg!(feature = "cuda") {
        return Some("feature flag".into());
    }
    // Check CUDA_PATH env var
    if let Ok(cuda_path) = env::var("CUDA_PATH") {
        if PathBuf::from(&cuda_path).exists() {
            return Some(format!("CUDA_PATH={}", cuda_path));
        }
    }
    // Check nvcc in PATH
    if std::process::Command::new("nvcc")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("nvcc in PATH".into());
    }
    // Check standard Windows path
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
    // Check standard Linux path
    if cfg!(target_os = "linux") {
        let cuda_path = PathBuf::from("/usr/local/cuda");
        if cuda_path.exists() {
            return Some("found at /usr/local/cuda".into());
        }
    }
    None
}

/// Returns true if Metal should be enabled (macOS only, or forced via feature flag).
fn detect_metal() -> bool {
    if cfg!(feature = "metal") {
        return true;
    }
    // Metal is available on all macOS targets
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    target_os == "macos"
}

/// Returns Some(reason) if Vulkan should be enabled, None otherwise.
fn detect_vulkan() -> Option<String> {
    // Feature flag force-override
    if cfg!(feature = "vulkan") {
        return Some("feature flag".into());
    }
    // Check VULKAN_SDK env var
    if let Ok(sdk_path) = env::var("VULKAN_SDK") {
        if PathBuf::from(&sdk_path).exists() {
            return Some(format!("VULKAN_SDK={}", sdk_path));
        }
    }
    None
}

/// Scan a directory for static library files and emit link directives for each.
fn link_all_static_libs(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Match .a (unix) or .lib (windows) files
        let lib_name = if let Some(name) = file_name
            .strip_prefix("lib")
            .and_then(|n| n.strip_suffix(".a"))
        {
            name.to_string()
        } else if let Some(name) = file_name.strip_suffix(".lib") {
            name.to_string()
        } else if let Some(name) = file_name.strip_suffix(".a") {
            // Some .a files may not have the lib prefix
            name.to_string()
        } else {
            continue;
        };

        println!("cargo:rustc-link-lib=static={}", lib_name);
    }
}
