use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sd_cpp_dir = manifest_dir.join("stable-diffusion.cpp");

    // --- CMake build ---
    let mut cmake_cfg = cmake::Config::new(&sd_cpp_dir);

    cmake_cfg
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("SD_BUILD_SHARED_LIB", "OFF")
        .define("SD_BUILD_EXAMPLES", "OFF");

    if cfg!(feature = "metal") {
        cmake_cfg.define("GGML_METAL", "ON");
    }
    if cfg!(feature = "cuda") {
        cmake_cfg.define("GGML_CUDA", "ON");
    }
    if cfg!(feature = "vulkan") {
        cmake_cfg.define("GGML_VULKAN", "ON");
    }

    // Read target info early (used for CMake config and linking)
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    // macOS: std::filesystem requires 10.15+
    if target_os == "macos" {
        cmake_cfg.define("CMAKE_OSX_DEPLOYMENT_TARGET", "10.15");
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
        }
        "macos" => {
            println!("cargo:rustc-link-lib=c++");
            if cfg!(feature = "metal") {
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
    if cfg!(feature = "cuda") {
        println!("cargo:rustc-link-lib=cudart");
        println!("cargo:rustc-link-lib=cublas");
        println!("cargo:rustc-link-lib=cublasLt");
    }

    // --- Vulkan system libraries ---
    if cfg!(feature = "vulkan") {
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
    let wrapper_h = manifest_dir.join("wrapper.h");
    let include_dir = sd_cpp_dir.join("include");

    let bindings = bindgen::Builder::default()
        .header(wrapper_h.to_str().unwrap())
        .clang_arg(format!("-I{}", include_dir.display()))
        // Force C enums to generate as top-level constants (type alias + pub const)
        .default_enum_style(bindgen::EnumVariation::Consts)
        // No allowlists — wrapper.h only includes stable-diffusion.h,
        // so all generated items are from sd.cpp's public API
        .generate()
        .expect("Failed to generate bindings");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Failed to write bindings");

    // --- Rerun triggers ---
    println!("cargo:rerun-if-changed=wrapper.h");
    println!(
        "cargo:rerun-if-changed={}",
        sd_cpp_dir.join("include").join("stable-diffusion.h").display()
    );
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
        let lib_name = if let Some(name) = file_name.strip_prefix("lib").and_then(|n| n.strip_suffix(".a")) {
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
