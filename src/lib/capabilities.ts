/**
 * What a model architecture can do beyond txt2img / img2img.
 *
 * Reference-image editing ("Edit Mode": describe a change, keep the subject)
 * only works on models trained for it. sd.cpp does not check: feeding a
 * reference image to Z-Image Turbo selects its Omni pipeline and crashes the
 * process with an access violation (reproduced 2026-09-03). The backend
 * refuses such requests too; this list is what the UI offers.
 */
export const EDIT_ARCHITECTURES: readonly string[] = ["flux-kontext"];

export function supportsEdit(architecture: string | undefined | null): boolean {
  return !!architecture && EDIT_ARCHITECTURES.includes(architecture);
}
