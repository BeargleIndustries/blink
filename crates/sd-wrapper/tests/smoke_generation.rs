//! End-to-end smoke test for the sd.cpp FFI migration.
//!
//! The generation test needs a real model on disk and a GPU, so it is marked
//! `#[ignore]`. Run it explicitly:
//!
//! ```text
//! cargo test -p sd-wrapper --test smoke_generation -- --ignored --nocapture
//! ```
//!
//! Model paths default to Blink's app-data directory and can be overridden with
//! `BLINK_TEST_MODEL_DIR`.
//!
//! # Do not read timings from a default run
//!
//! These tests are for correctness. Two things make their wall-clock times useless as
//! benchmarks, and both have already produced wrong conclusions once:
//!
//! 1. **Cargo runs tests in parallel.** Both GPU tests contend for the same device, which
//!    inflated one generation from ~6 s to ~77 s. Use `--test-threads=1` when timing.
//! 2. **The OS page cache dominates.** The first generation after boot reads ~8.5 GB of
//!    weights from disk (~154 s observed); every run after that is warm (~6 s). Comparing
//!    a cold run against a warm one measures the file cache, not whatever you changed.
//!
//! To compare configurations, run each one repeatedly and alternating, with
//! `--test-threads=1`, discarding the first run of the session.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Output sanity checking
// ---------------------------------------------------------------------------

/// Why a generated image looks degenerate, or `None` if it looks like a real image.
///
/// This guards the specific failure modes this codebase has actually hit: an all-black
/// frame (the VAE decoding on the wrong backend) and a flat/uniform frame (weights not
/// applied). A migration that compiles but silently produces black output must fail here.
fn degeneracy_reason(rgba: &[u8], width: u32, height: u32) -> Option<String> {
    let expected = (width as usize) * (height as usize) * 4;
    if rgba.len() != expected {
        return Some(format!(
            "buffer is {} bytes, expected {} for {}x{} RGBA",
            rgba.len(),
            expected,
            width,
            height
        ));
    }
    if rgba.is_empty() {
        return Some("buffer is empty".to_string());
    }

    // Look at colour channels only; a fully opaque alpha channel is expected and
    // would otherwise mask a black image as "has variation".
    let colour: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();

    let nonzero = colour.iter().filter(|&&b| b != 0).count();
    if nonzero == 0 {
        return Some("image is entirely black (all colour channels zero)".to_string());
    }

    let mut seen = [false; 256];
    for &b in &colour {
        seen[b as usize] = true;
    }
    let distinct = seen.iter().filter(|&&s| s).count();
    if distinct < 8 {
        return Some(format!(
            "image has only {} distinct colour values — looks flat, not a render",
            distinct
        ));
    }

    let mean = colour.iter().map(|&b| b as f64).sum::<f64>() / colour.len() as f64;
    let variance =
        colour.iter().map(|&b| (b as f64 - mean).powi(2)).sum::<f64>() / colour.len() as f64;
    let stddev = variance.sqrt();
    if stddev < 3.0 {
        return Some(format!(
            "image standard deviation is {:.2} — nearly uniform, not a render",
            stddev
        ));
    }

    None
}

#[cfg(test)]
mod degeneracy_tests {
    use super::degeneracy_reason;

    /// 2x2 RGBA buffer built from a per-pixel colour function.
    fn buf(f: impl Fn(usize) -> [u8; 3]) -> Vec<u8> {
        (0..4)
            .flat_map(|i| {
                let [r, g, b] = f(i);
                [r, g, b, 255]
            })
            .collect()
    }

    #[test]
    fn all_black_is_degenerate() {
        let img = buf(|_| [0, 0, 0]);
        let reason = degeneracy_reason(&img, 2, 2).expect("all-black must be flagged");
        assert!(reason.contains("black"), "unexpected reason: {reason}");
    }

    #[test]
    fn opaque_alpha_does_not_mask_a_black_image() {
        // Alpha is 255 everywhere; only colour channels are zero.
        let img = buf(|_| [0, 0, 0]);
        assert!(
            degeneracy_reason(&img, 2, 2).is_some(),
            "a black image with opaque alpha must still be flagged"
        );
    }

    #[test]
    fn uniform_grey_is_degenerate() {
        let img = buf(|_| [128, 128, 128]);
        let reason = degeneracy_reason(&img, 2, 2).expect("uniform image must be flagged");
        assert!(
            reason.contains("flat") || reason.contains("uniform"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn wrong_buffer_length_is_reported() {
        let reason = degeneracy_reason(&[0, 0, 0, 255], 2, 2).expect("size mismatch must be flagged");
        assert!(reason.contains("expected"), "unexpected reason: {reason}");
    }

    #[test]
    fn varied_image_is_accepted() {
        // Spread values widely so both the distinct-count and stddev checks pass.
        let img: Vec<u8> = (0..64u32)
            .flat_map(|i| {
                let v = (i * 4) as u8;
                [v, 255 - v, v.wrapping_mul(3), 255]
            })
            .collect();
        assert_eq!(
            degeneracy_reason(&img, 8, 8),
            None,
            "a varied image must not be flagged"
        );
    }
}

// ---------------------------------------------------------------------------
// Real generation smoke test
// ---------------------------------------------------------------------------

fn model_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("BLINK_TEST_MODEL_DIR") {
        return PathBuf::from(dir);
    }
    let appdata = std::env::var("APPDATA").expect("APPDATA not set");
    PathBuf::from(appdata).join("com.beargle.blink").join("models")
}

/// The parameter placement a timing run is exercising.
struct Placement {
    label: &'static str,
    auto_fit: bool,
    low_memory: bool,
}

/// Read `BLINK_TEST_PLACEMENT`: `auto_fit` (default) | `low_memory` | `resident`.
///
/// This is the knob the item-2 measurement gate alternates between. `resident`
/// means neither flag, i.e. sd.cpp's own default placement.
fn placement_from_env() -> Placement {
    match std::env::var("BLINK_TEST_PLACEMENT").as_deref() {
        Ok("low_memory") => Placement { label: "low_memory", auto_fit: false, low_memory: true },
        Ok("resident") => Placement { label: "resident", auto_fit: false, low_memory: false },
        Ok("auto_fit") | Err(_) => Placement { label: "auto_fit", auto_fit: true, low_memory: false },
        Ok(other) => panic!(
            "BLINK_TEST_PLACEMENT={other:?} is not one of auto_fit | low_memory | resident"
        ),
    }
}

/// Generate a real image through the migrated FFI path.
///
/// Z-Image is deliberately chosen: it is the stack Blink used to force onto the CPU
/// with an architecture-name rule, so it is the one whose parameter placement the
/// move to sd.cpp's `auto_fit` actually changes.
///
/// `BLINK_TEST_PLACEMENT` selects the placement under test:
/// `auto_fit` (default) — sd.cpp decides from measured VRAM;
/// `low_memory` — Blink's pre-auto_fit behaviour, `params_backend=cpu`;
/// `resident` — neither, i.e. sd.cpp's own default placement.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn zimage_txt2img_produces_a_real_image() {
    use sd_wrapper::{ContextConfig, GenerationParams, LoraApplyMode, SampleMethod, SdContext};

    let dir = model_dir();
    let diffusion = dir.join("z-image").join("z_image_turbo-Q8_0.gguf");
    let llm = dir.join("z-image").join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    let vae = dir.join("z-image").join("ae.safetensors");

    for (label, path) in [("diffusion", &diffusion), ("llm", &llm), ("vae", &vae)] {
        assert!(
            path.exists(),
            "missing {label} model at {}. Set BLINK_TEST_MODEL_DIR to override.",
            path.display()
        );
    }

    let placement = placement_from_env();
    let config = ContextConfig {
        model_path: None,
        vae_path: Some(vae.to_string_lossy().into_owned()),
        clip_l_path: None,
        t5xxl_path: None,
        diffusion_model_path: Some(diffusion.to_string_lossy().into_owned()),
        llm_path: Some(llm.to_string_lossy().into_owned()),
        n_threads: 4,
        auto_fit: placement.auto_fit,
        low_memory: placement.low_memory,
        control_net_path: None,
        taesd_path: None,
        lora_apply_mode: LoraApplyMode::Auto,
    };

    eprintln!(
        "[smoke] placement={} (auto_fit={}, low_memory={})",
        placement.label, config.auto_fit, config.low_memory
    );

    let started = std::time::Instant::now();
    let ctx = SdContext::new(config).expect("failed to create sd.cpp context");
    eprintln!("[smoke] context loaded in {:?}", started.elapsed());

    // Z-Image Turbo: cfg 1.0, few steps, euler (per sd.cpp docs/z_image.md).
    let params = GenerationParams {
        prompt: "a red apple on a wooden table, studio lighting".to_string(),
        width: 512,
        height: 512,
        steps: 4,
        cfg_scale: 1.0,
        seed: 42,
        sample_method: SampleMethod::Euler,
        ..Default::default()
    };

    let gen_started = std::time::Instant::now();
    let image = ctx
        .txt2img(params, Vec::new(), None, None)
        .expect("txt2img failed");
    eprintln!("[smoke] generated in {:?}", gen_started.elapsed());

    assert_eq!(image.width, 512, "unexpected output width");
    assert_eq!(image.height, 512, "unexpected output height");

    if let Some(reason) = degeneracy_reason(&image.data, image.width, image.height) {
        panic!("generated image failed sanity check: {reason}");
    }

    // Write it out so the result can be eyeballed.
    // Name the file after the placement mode so successive runs do not overwrite
    // each other — the outputs need to be comparable side by side.
    let suffix = placement.label;

    // Checksum the raw RGBA (not the PNG) so the comparison is not affected by
    // encoder settings or metadata.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325; // FNV-1a
    for &b in &image.data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    eprintln!("[smoke] rgba_fnv1a={hash:016x} bytes={}", image.data.len());

    let out = std::env::temp_dir().join(format!("blink_smoke_zimage_{suffix}.png"));
    image::save_buffer(
        &out,
        &image.data,
        image.width,
        image.height,
        image::ColorType::Rgba8,
    )
    .expect("failed to write smoke-test image");
    eprintln!("[smoke] wrote {}", out.display());
}

/// Two different seeds must produce two different images.
///
/// This is the counterpart to the fixed-seed test above. That one proves determinism;
/// this one proves *liveness* — that generation is not handing back a cached or stale
/// buffer. It matters specifically because the sd.cpp bump changed how result memory is
/// allocated and released (`free_sd_images` replacing manual `libc::free`), and a
/// use-after-free or a reused buffer could easily still produce a valid-looking image.
///
/// A fixed-seed test cannot catch that failure mode; identical output is its pass
/// condition.
#[test]
#[ignore = "requires a downloaded Z-Image model and a GPU"]
fn different_seeds_produce_different_images() {
    use sd_wrapper::{ContextConfig, GenerationParams, LoraApplyMode, SampleMethod, SdContext};

    let dir = model_dir();
    let diffusion = dir.join("z-image").join("z_image_turbo-Q8_0.gguf");
    let llm = dir.join("z-image").join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf");
    let vae = dir.join("z-image").join("ae.safetensors");
    if !diffusion.exists() || !llm.exists() || !vae.exists() {
        panic!("Z-Image model files missing; set BLINK_TEST_MODEL_DIR");
    }

    let config = ContextConfig {
        model_path: None,
        vae_path: Some(vae.to_string_lossy().into_owned()),
        clip_l_path: None,
        t5xxl_path: None,
        diffusion_model_path: Some(diffusion.to_string_lossy().into_owned()),
        llm_path: Some(llm.to_string_lossy().into_owned()),
        n_threads: 4,
        auto_fit: true,
        low_memory: false,
        control_net_path: None,
        taesd_path: None,
        lora_apply_mode: LoraApplyMode::Auto,
    };

    let ctx = SdContext::new(config).expect("failed to create sd.cpp context");

    let gen = |seed: i64| {
        let params = GenerationParams {
            prompt: "a red apple on a wooden table, studio lighting".to_string(),
            width: 512,
            height: 512,
            steps: 4,
            cfg_scale: 1.0,
            seed,
            sample_method: SampleMethod::Euler,
            ..Default::default()
        };
        ctx.txt2img(params, Vec::new(), None, None)
            .unwrap_or_else(|e| panic!("txt2img failed for seed {seed}: {e:?}"))
    };

    let a = gen(42);
    let b = gen(1337);

    // Both must independently be real images, not just different from each other.
    for (seed, img) in [(42, &a), (1337, &b)] {
        if let Some(reason) = degeneracy_reason(&img.data, img.width, img.height) {
            panic!("seed {seed} produced a degenerate image: {reason}");
        }
    }

    assert_ne!(
        a.data, b.data,
        "seeds 42 and 1337 produced identical pixel data — generation is returning a \
         stale or cached buffer rather than rendering"
    );

    let differing = a
        .data
        .iter()
        .zip(b.data.iter())
        .filter(|(x, y)| x != y)
        .count();
    let frac = differing as f64 / a.data.len() as f64;
    eprintln!("[smoke] seeds 42 vs 1337 differ in {:.1}% of bytes", frac * 100.0);
    assert!(
        frac > 0.10,
        "only {:.2}% of bytes differ between seeds — suspiciously similar",
        frac * 100.0
    );
}

// ---------------------------------------------------------------------------
// LoRA strength probe
// ---------------------------------------------------------------------------

/// Fraction of colour bytes clipped to 0 or 255 — an "overcooked" signal.
///
/// A LoRA applied several times stronger than intended typically saturates: blown
/// highlights and crushed blacks. This gives a number to compare instead of an opinion.
fn clipped_fraction(rgba: &[u8]) -> f64 {
    let colour: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let clipped = colour.iter().filter(|&&b| b == 0 || b == 255).count();
    clipped as f64 / colour.len() as f64
}

/// Mean absolute per-byte difference between two images, 0-255.
fn mean_abs_diff(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs() as f64)
        .sum::<f64>()
        / a.len() as f64
}

/// Measure how strongly a LoRA is actually applied, to decide whether the
/// "Z-Image applies LoRA 4x" workaround in `src-tauri/src/commands/generation.rs`
/// is still needed after the sd.cpp bump.
///
/// Renders the same seed with no LoRA, then at several multipliers, and reports
/// divergence from baseline plus a clipping ratio. Run with `--nocapture` and read
/// the table; sd.cpp's own log lines about applied/unmatched tensors are the other
/// half of the evidence.
///
/// Point it at a LoRA with `BLINK_TEST_LORA=<path>`.
#[test]
#[ignore = "diagnostic probe; needs a Z-Image model and a LoRA"]
fn lora_strength_probe() {
    use sd_wrapper::{ContextConfig, GenerationParams, LoraApplyMode, LoraConfig, SampleMethod, SdContext};

    let lora_path = match std::env::var("BLINK_TEST_LORA") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("[probe] set BLINK_TEST_LORA=<path to .safetensors> to run");
            return;
        }
    };
    assert!(
        std::path::Path::new(&lora_path).exists(),
        "LoRA not found: {lora_path}"
    );

    let dir = model_dir();
    let config = ContextConfig {
        model_path: None,
        vae_path: Some(dir.join("z-image").join("ae.safetensors").to_string_lossy().into_owned()),
        clip_l_path: None,
        t5xxl_path: None,
        diffusion_model_path: Some(
            dir.join("z-image").join("z_image_turbo-Q8_0.gguf").to_string_lossy().into_owned(),
        ),
        llm_path: Some(
            dir.join("z-image")
                .join("Qwen3-4B-Instruct-2507-Q4_K_M.gguf")
                .to_string_lossy()
                .into_owned(),
        ),
        n_threads: 4,
        auto_fit: true,
        low_memory: false,
        control_net_path: None,
        taesd_path: None,
        // BLINK_TEST_LORA_MODE=immediate|runtime|auto (default auto)
        lora_apply_mode: match std::env::var("BLINK_TEST_LORA_MODE").as_deref() {
            Ok("immediate") => LoraApplyMode::Immediately,
            Ok("runtime") => LoraApplyMode::AtRuntime,
            _ => LoraApplyMode::Auto,
        },
    };
    eprintln!("[probe] lora_apply_mode={:?}", config.lora_apply_mode);

    let ctx = SdContext::new(config).expect("context creation failed");

    let render = |mult: Option<f32>| {
        let loras = match mult {
            Some(m) => vec![LoraConfig {
                path: lora_path.clone(),
                multiplier: m,
                is_high_noise: false,
            }],
            None => Vec::new(),
        };
        let params = GenerationParams {
            prompt: "a portrait photograph of a woman, natural light".to_string(),
            width: 512,
            height: 512,
            steps: 4,
            cfg_scale: 1.0,
            seed: 42,
            sample_method: SampleMethod::Euler,
            loras,
            ..Default::default()
        };
        ctx.txt2img(params, Vec::new(), None, None)
            .unwrap_or_else(|e| panic!("txt2img failed at {mult:?}: {e:?}"))
    };

    let base = render(None);
    eprintln!(
        "[probe] baseline (no lora): clipped={:.2}%",
        clipped_fraction(&base.data) * 100.0
    );

    for mult in [0.25f32, 0.5, 1.0] {
        let img = render(Some(mult));
        let diff = mean_abs_diff(&base.data, &img.data);
        let clip = clipped_fraction(&img.data) * 100.0;
        let same = img.data == base.data;
        eprintln!(
            "[probe] mult={mult:<5} diff_from_base={diff:6.2}/255  clipped={clip:5.2}%  \
             identical_to_base={same}"
        );
        let out = std::env::temp_dir().join(format!("blink_lora_probe_{mult}.png"));
        image::save_buffer(&out, &img.data, img.width, img.height, image::ColorType::Rgba8)
            .expect("save failed");
    }
    let out = std::env::temp_dir().join("blink_lora_probe_base.png");
    image::save_buffer(&out, &base.data, base.width, base.height, image::ColorType::Rgba8)
        .expect("save failed");
    eprintln!("[probe] images written to {}", std::env::temp_dir().display());
}
