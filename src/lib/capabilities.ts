/**
 * What a model can do beyond plain txt2img, driven by the `features` list on
 * each catalog entry (models.json) and mirrored for custom imports in the
 * backend's `architecture_features`.
 *
 * Reference-image editing ("Edit Mode": describe a change, keep the subject)
 * only works on models trained for it. sd.cpp does not check: feeding a
 * reference image to Z-Image Turbo selects its Omni pipeline and crashes the
 * process with an access violation (reproduced 2026-09-03). The backend
 * refuses such requests too (EDIT_ARCHITECTURES in generation.rs).
 */
import type { ModelInfo } from "./types";

/** Fallback for models with no `features` list (e.g. custom imports). */
export const EDIT_ARCHITECTURES: readonly string[] = ["flux-kontext", "flux2-klein", "flux2-klein-9b"];

export function supportsEdit(model: ModelInfo | null | undefined): boolean {
  if (!model) return false;
  if (model.features?.length) return model.features.includes("edit");
  return EDIT_ARCHITECTURES.includes(model.architecture);
}

export function supportsVideo(model: ModelInfo | null | undefined): boolean {
  return !!model?.features?.includes("video");
}

/** Badge text and colour per feature, in display order. */
export const FEATURE_BADGES: Record<string, { label: string; title: string; color: string }> = {
  txt2img: { label: "Text → Image", title: "Generates images from a prompt", color: "var(--text-secondary)" },
  img2img: { label: "Image → Image", title: "Transforms or inpaints an input image", color: "var(--text-secondary)" },
  edit: { label: "✎ Edit", title: "Edit Mode: describe a change and keep the subject (reference-image editing)", color: "#a78bfa" },
  video: { label: "🎬 Video", title: "Generates short video clips", color: "#f472b6" },
  i2v: { label: "Image → Video", title: "Animates an input image", color: "#f472b6" },
  upscale: { label: "⤢ Upscale", title: "Upscaler, used from the Upscale button", color: "#34d399" },
  controlnet: { label: "ControlNet", title: "Structural conditioning add-on", color: "#34d399" },
  preview: { label: "Preview decoder", title: "Fast preview decoder used during generation", color: "#34d399" },
  fast: { label: "⚡ Fast", title: "Step-distilled: a few steps per image", color: "#fbbf24" },
  text: { label: "Aa Text", title: "Renders readable text in images", color: "#60a5fa" },
  anime: { label: "Anime", title: "Trained for anime / illustration styles", color: "#fb7185" },
};

export const FEATURE_ORDER = Object.keys(FEATURE_BADGES);
