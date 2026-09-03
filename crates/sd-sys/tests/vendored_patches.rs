//! CPU-only guards on the local patches we carry against vendored
//! stable-diffusion.cpp. These run in the plain `cargo test --workspace` sweep
//! with no GPU and no model, so CI catches a missing patch even if someone
//! disables the build-script step.

use std::path::PathBuf;

fn sd_cpp_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("stable-diffusion.cpp")
}

fn patches_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("patches")
}

/// sd.cpp's `convert_sep_to_dot()` rewrites `_` to `.` unless a token is in
/// `protected_tokens`. Without `feed_forward` there, kohya-style LoRA FFN
/// tensor names are mangled and silently dropped ("unused lora tensor"), so
/// the LoRA only half-applies. This asserts the fix is in the tree that
/// actually gets compiled.
#[test]
fn feed_forward_is_a_protected_token() {
    let path = sd_cpp_dir().join("src/name_conversion.cpp");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    assert!(
        source.contains("\"feed_forward\","),
        "{} does not contain \"feed_forward\", — the LoRA FFN fix is missing from the vendored \
         sources. It is applied at build time from crates/sd-sys/patches/; run `cargo build -p \
         sd-sys`, and check that BLINK_SKIP_SD_PATCHES is not set.",
        path.display()
    );

    assert!(
        !source.contains("<<<<<<<") && !source.contains(">>>>>>>"),
        "{} contains merge conflict markers; the vendored tree is corrupt. Run: git -C \
         crates/sd-sys/stable-diffusion.cpp checkout -- src/name_conversion.cpp",
        path.display()
    );
}

/// Mechanical enforcement of the fork-revisit trigger. Carrying more than two
/// local patches is the point at which chaining `git apply` gets fragile and a
/// Blink-owned fork of stable-diffusion.cpp becomes the cheaper option
/// (see .omc/plans/first-release-sdcpp-6b3edaa.md, RALPLAN-DR Q1, Option A).
#[test]
fn we_are_not_carrying_too_many_local_patches() {
    let dir = patches_dir();
    let count = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("patch"))
        .count();

    assert!(
        count <= 2,
        "{count} local sd.cpp patches in {} — the limit is 2. Adding a third is the agreed \
         trigger to stop patching at build time and move to a Blink-owned fork of \
         stable-diffusion.cpp with the submodule repointed at it (RALPLAN-DR Q1, Option A, in \
         .omc/plans/first-release-sdcpp-6b3edaa.md). Make that decision deliberately rather than \
         raising this number.",
        dir.display()
    );
}
