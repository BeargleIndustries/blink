use std::env;
use std::path::{Path, PathBuf};

/// The vendored source file the `feed_forward` fix lives in, relative to the
/// submodule root. Checked as a postcondition on every path except the explicit
/// `BLINK_SKIP_SD_PATCHES=1` opt-out, so that opt-out is the only way a build
/// can succeed without the LoRA FFN fix present.
const FEED_FORWARD_FILE: &str = "src/name_conversion.cpp";
/// The exact text the fix adds to `protected_tokens`.
const FEED_FORWARD_TOKEN: &str = "\"feed_forward\",";

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sd_cpp_dir = manifest_dir.join("stable-diffusion.cpp");

    // --- Local patches against the pristine upstream submodule ---
    // The submodule stays pinned to an unmodified upstream SHA; our local fixes
    // are applied to its working tree here. A permanently dirty submodule
    // working tree is the expected state (see docs/upgrade-research-2026-09.md).
    apply_vendored_patches(&manifest_dir, &sd_cpp_dir);

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
            // ggml's CPU backend reads CPU topology from the registry
            // (ggml_backend_cpu_device_context uses RegOpenKeyExA /
            // RegQueryValueExA / RegCloseKey), which live in advapi32.
            println!("cargo:rustc-link-lib=advapi32");
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
    // Watch the vendored C++ sources, not just the header. Without this cargo
    // considers sd-sys up to date after a submodule bump or a local patch to
    // stable-diffusion.cpp, silently skipping the build script and linking a
    // stale library — the change appears to have no effect.
    println!("cargo:rerun-if-changed={}", header_path.display());
    println!("cargo:rerun-if-changed={}", sd_cpp_dir.join("src").display());
    println!("cargo:rerun-if-changed={}", sd_cpp_dir.join("include").display());
    println!(
        "cargo:rerun-if-changed={}",
        sd_cpp_dir.join("CMakeLists.txt").display()
    );
    println!("cargo:rerun-if-env-changed=CUDA_PATH");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
}

/// Apply `crates/sd-sys/patches/*.patch` to the vendored stable-diffusion.cpp
/// working tree, idempotently, before CMake sees the sources.
///
/// Why this exists: the submodule is pinned to a pristine upstream SHA so that
/// bumping it stays a one-line change and reviewers can see exactly which
/// upstream revision is in use. Our local fixes (currently just the
/// `feed_forward` protected-token fix that stops LoRA FFN tensors being
/// dropped) live as patch files next to this build script — the same artifact
/// we send upstream. When upstream merges one, the patch stops applying and
/// this build fails loudly with a message telling you to delete it.
///
/// ## Why plain `git apply`, and not `git apply --3way`
///
/// `--3way` is tempting for line-ending robustness but is wrong here, verified
/// on this repo with git 2.52 and `core.autocrlf=true`:
///
/// * `--3way` implies `--index`, so it *stages* the change inside the
///   submodule. The expected state is an unstaged ` M`, not a staged `M `.
/// * `git apply --3way --check` succeeds in *both* directions (forward and
///   `--reverse`) because the blob-level merge is trivially satisfiable, which
///   destroys the already-applied/not-yet-applied discrimination below.
/// * `--3way` can leave conflict markers in the working file on a partial
///   failure; plain `git apply` is all-or-nothing.
///
/// Nor do we pass `-c core.autocrlf=false`: with `autocrlf=true` the working
/// tree is CRLF while the index blob is LF, and disabling the conversion makes
/// every apply fail (`does not match index` / `patch does not apply`). Letting
/// git use its configured conversion is what makes an LF patch apply cleanly to
/// a CRLF checkout. The patch files themselves are pinned to LF by the
/// repo-root `.gitattributes` (`*.patch -text`), which is the half of the CRLF
/// story that does need to be forced.
fn apply_vendored_patches(manifest_dir: &Path, sd_cpp_dir: &Path) {
    let patch_dir = manifest_dir.join("patches");
    println!("cargo:rerun-if-changed={}", patch_dir.display());
    println!("cargo:rerun-if-env-changed=BLINK_SKIP_SD_PATCHES");

    if env::var("BLINK_SKIP_SD_PATCHES").as_deref() == Ok("1") {
        println!(
            "cargo:warning=BLINK_SKIP_SD_PATCHES is set — vendored sd.cpp patches are NOT \
             applied; LoRA feed_forward tensors will be dropped"
        );
        return;
    }

    let mut patches: Vec<PathBuf> = std::fs::read_dir(&patch_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("patch"))
        .collect();
    // Sort by filename so application order is deterministic (0001-, 0002-, ...).
    patches.sort();

    if patches.is_empty() {
        // Nothing to apply. That is the steady state after upstream merges every
        // local fix — and it is also exactly what an accidental deletion looks
        // like, so this does NOT return: the postcondition below still runs.
        println!(
            "cargo:warning=no sd.cpp patches found in {}",
            patch_dir.display()
        );
    }

    for patch in &patches {
        let name = patch
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unnamed>")
            .to_string();

        // i. Already applied? A clean *reverse* apply means the change is
        //    already in the working tree, so repeated builds are a no-op and
        //    the source file's mtime stays stable.
        if git_apply(sd_cpp_dir, &["--check", "--reverse"], patch).0 {
            println!("cargo:warning=sd.cpp patch already applied: {name}");
            continue;
        }

        // ii. Applicable? If not, work out *why* before panicking: an upstream
        //     merge and a broken patch need opposite responses, and confusing
        //     them is how the fix gets deleted by mistake.
        let (ok, stderr) = git_apply(sd_cpp_dir, &["--check"], patch);
        if !ok {
            if patch_additions_already_present(sd_cpp_dir, patch) {
                panic!(
                    "Patch `{name}` no longer applies, and its changes are already present in \
                     the vendored stable-diffusion.cpp sources. Upstream has merged it — delete \
                     `crates/sd-sys/patches/{name}`."
                );
            }
            panic!(
                "Patch `{name}` failed to apply to submodule {sha} and the fix is NOT present. \
                 This is a patch failure, not an upstream merge. Do not delete the patch. \
                 git stderr: {stderr}\nCheck line endings (`core.autocrlf` may be true on this \
                 host; `*.patch` must stay LF) and rebase the patch onto the new submodule \
                 revision.",
                sha = submodule_short_sha(sd_cpp_dir),
            );
        }

        // iii. Apply. On failure restore the touched files first — never leave
        //      a half-patched vendored tree behind for the next build to
        //      misread.
        let (ok, stderr) = git_apply(sd_cpp_dir, &[], patch);
        if !ok {
            let restore_note = restore_patch_targets(sd_cpp_dir, patch);
            panic!(
                "Patch `{name}` passed --check but failed to apply to submodule {sha}. \
                 git stderr: {stderr}{restore_note}",
                sha = submodule_short_sha(sd_cpp_dir),
            );
        }
        println!("cargo:warning=applied sd.cpp patch: {name}");
    }

    check_feed_forward_postcondition(sd_cpp_dir);
}

/// Postcondition, independent of git's exit codes and of whether any patch was
/// applied: a patch that landed on the wrong hunk, a refactor that moved the
/// protected-token list, and a deleted patch file all reach here.
fn check_feed_forward_postcondition(sd_cpp_dir: &Path) {
    let target = sd_cpp_dir.join(FEED_FORWARD_FILE);
    let source = std::fs::read_to_string(&target)
        .unwrap_or_else(|e| panic!("failed to read {} after patching: {e}", target.display()));

    // Corruption is checked FIRST: a tree carrying conflict markers usually also
    // fails the token check, and "the tree is corrupt, restore it" is the
    // actionable answer — "the fix is not present" would send the reader off to
    // rebase a patch that is fine.
    if source.contains("<<<<<<<") || source.contains(">>>>>>>") {
        panic!(
            "{FEED_FORWARD_FILE} contains merge conflict markers; the vendored tree is corrupt. \
             Run: git -C crates/sd-sys/stable-diffusion.cpp checkout -- {FEED_FORWARD_FILE} \
             and rebuild"
        );
    }
    if !source.contains(FEED_FORWARD_TOKEN) {
        panic!(
            "{FEED_FORWARD_FILE} does not contain {FEED_FORWARD_TOKEN} — the LoRA FFN fix is not \
             in this build, so LoRA feed-forward tensors would be silently dropped. Either a \
             patch applied to the wrong hunk, or `crates/sd-sys/patches/` no longer carries the \
             fix; deleting that patch is only correct once the token is present upstream. Set \
             BLINK_SKIP_SD_PATCHES=1 to build without the fix deliberately."
        );
    }
}

/// Run `git -C <sd_cpp_dir> apply <extra args> <patch>`. Returns
/// (success, stderr). Panics only if git itself cannot be spawned — a missing
/// git must never degrade into a silent skip.
fn git_apply(sd_cpp_dir: &Path, extra: &[&str], patch: &Path) -> (bool, String) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(sd_cpp_dir)
        .arg("apply")
        .args(extra)
        .arg(patch)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "git is required to apply vendored sd.cpp patches (it is already required for \
                 the submodule checkout): {e}"
            )
        });

    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    )
}

/// Files a unified diff touches, taken from its `+++ b/<path>` headers.
fn patch_targets(patch: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(patch) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| l.strip_prefix("+++ b/"))
        .map(|p| p.trim().to_string())
        .collect()
}

/// True when every line the patch *adds* is already present in the submodule's
/// **committed** (`HEAD`) sources — i.e. upstream has merged our fix.
///
/// This deliberately reads `HEAD:<path>` rather than the working tree. The
/// working tree normally already contains the addition because *we* put it
/// there on a previous build, so a working-tree grep answers "yes, upstream
/// merged it" for a merely-broken patch and invites someone to delete a patch
/// that is still the only source of the fix (the exact way the fix gets lost).
/// `HEAD` contains the addition only if it really came from upstream.
fn patch_additions_already_present(sd_cpp_dir: &Path, patch: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(patch) else {
        return false;
    };

    let mut current: Option<String> = None;
    let mut checked_any = false;

    for line in text.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current = committed_source(sd_cpp_dir, path.trim());
            continue;
        }
        if line.starts_with("+++") || !line.starts_with('+') {
            continue;
        }
        let added = &line[1..];
        if added.trim().is_empty() {
            continue;
        }
        checked_any = true;
        match current.as_deref() {
            Some(source) if source.contains(added.trim()) => {}
            _ => return false,
        }
    }

    checked_any
}

/// Contents of `<path>` as committed at the submodule's `HEAD`, i.e. pristine
/// upstream without any of our local patches.
fn committed_source(sd_cpp_dir: &Path, path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(sd_cpp_dir)
        .arg("show")
        .arg(format!("HEAD:{path}"))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Discard any partial write left by a failed apply. Returns a note to append
/// to the panic message — a vendored tree we could not restore must be
/// reported, never left silently in place.
fn restore_patch_targets(sd_cpp_dir: &Path, patch: &Path) -> String {
    let mut failures = Vec::new();
    for target in patch_targets(patch) {
        let restored = std::process::Command::new("git")
            .arg("-C")
            .arg(sd_cpp_dir)
            .args(["checkout", "--"])
            .arg(&target)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !restored {
            failures.push(target);
        }
    }
    if failures.is_empty() {
        String::new()
    } else {
        format!(
            "\nWARNING: could not restore {} — the vendored tree may be corrupt. Run: \
             git -C crates/sd-sys/stable-diffusion.cpp checkout -- {}",
            failures.join(", "),
            failures.join(" "),
        )
    }
}

/// Short SHA of the vendored submodule, for patch-failure messages.
fn submodule_short_sha(sd_cpp_dir: &Path) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(sd_cpp_dir)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "<unknown>".into())
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
