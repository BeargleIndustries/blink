import { invoke } from "@tauri-apps/api/core";
import type {
  GenerationRequest,
  ModelInfo,
  DownloadProgress,
  SystemInfo,
  AppSettings,
  GalleryItem,
  PerfSettings,
  VideoRequest,
  LoraFileInfo,
} from "./types";

export async function generateImage(request: GenerationRequest): Promise<string> {
  return invoke("generate_image", { request });
}

export async function cancelGeneration(): Promise<void> {
  return invoke("cancel_generation");
}

export async function getModels(): Promise<ModelInfo[]> {
  return invoke("get_models");
}

export async function getDownloadedModels(): Promise<ModelInfo[]> {
  return invoke("get_downloaded_models");
}

export async function downloadModel(modelId: string): Promise<void> {
  return invoke("download_model", { modelId });
}

export async function deleteModel(modelId: string): Promise<void> {
  return invoke("delete_model", { modelId });
}

export async function setActiveModel(modelId: string): Promise<void> {
  return invoke("set_active_model", { modelId });
}

export async function getDownloadProgress(modelId: string): Promise<DownloadProgress | null> {
  return invoke("get_download_progress", { modelId });
}

export async function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<void> {
  return invoke("save_settings", { settings });
}

export async function getSystemInfo(): Promise<SystemInfo> {
  return invoke("get_system_info");
}

export async function getAppVersion(): Promise<string> {
  return invoke("get_app_version");
}

export async function getLicenses(): Promise<string> {
  return invoke("get_licenses");
}

export async function getGallery(): Promise<GalleryItem[]> {
  return invoke("get_gallery");
}

export async function deleteGalleryItem(itemId: string): Promise<void> {
  return invoke("delete_gallery_item", { itemId });
}

export async function loadGalleryImage(itemId: string): Promise<string> {
  return invoke("load_gallery_image", { itemId });
}

export async function exportImage(itemId: string, destination: string): Promise<void> {
  return invoke("export_image", { itemId, destination });
}

export interface SaveToGalleryParams {
  imageBase64: string;
  prompt: string;
  negativePrompt: string;
  modelId: string;
  modelName: string;
  width: number;
  height: number;
  steps: number;
  cfgScale: number;
  seed: number;
  sampler: string;
  generationTimeSecs: number;
}

export async function saveToGallery(params: SaveToGalleryParams): Promise<GalleryItem> {
  return invoke("save_to_gallery", {
    imageBase64: params.imageBase64,
    prompt: params.prompt,
    negativePrompt: params.negativePrompt,
    modelId: params.modelId,
    modelName: params.modelName,
    width: params.width,
    height: params.height,
    steps: params.steps,
    cfgScale: params.cfgScale,
    seed: params.seed,
    sampler: params.sampler,
    generationTimeSecs: params.generationTimeSecs,
  });
}

export async function importCustomModel(url: string, name?: string): Promise<void> {
  return invoke("import_custom_model", { url, name: name ?? null });
}

export async function getPerfSettings(): Promise<PerfSettings> {
  return invoke("get_perf_settings");
}

export async function savePerfSettings(settings: PerfSettings): Promise<void> {
  return invoke("save_perf_settings", { settings });
}

export async function getHfToken(): Promise<string | null> {
  return invoke("get_hf_token");
}

export async function setHfToken(token: string | null): Promise<void> {
  return invoke("set_hf_token", { token });
}

export async function loadUpscaler(modelPath: string): Promise<void> {
  return invoke("load_upscaler", { modelPath });
}

export async function upscaleImage(imageBase64: string, factor: number): Promise<string> {
  return invoke("upscale_image", { imageBase64, factor });
}

export async function unloadUpscaler(): Promise<void> {
  return invoke("unload_upscaler");
}

export async function generateVideo(request: VideoRequest): Promise<string> {
  return invoke("generate_video", { request });
}

export async function scanLoraDirectory(path: string): Promise<LoraFileInfo[]> {
  return invoke("scan_lora_directory", { path });
}

export async function enhancePrompt(prompt: string, apiKey: string): Promise<{ prompt: string; negative_prompt: string }> {
  return invoke("enhance_prompt", { prompt, apiKey });
}

export async function getAnthropicKey(): Promise<string | null> {
  return invoke("get_anthropic_key");
}

export async function setAnthropicKey(key: string | null): Promise<void> {
  return invoke("set_anthropic_key", { key });
}

export async function enhancePromptLocal(prompt: string, endpoint: string, model: string): Promise<{ prompt: string; negative_prompt: string }> {
  return invoke("enhance_prompt_local", { prompt, endpoint, model });
}

export async function getLocalLlmEndpoint(): Promise<string | null> {
  return invoke("get_local_llm_endpoint");
}

export async function setLocalLlmEndpoint(endpoint: string | null): Promise<void> {
  return invoke("set_local_llm_endpoint", { endpoint });
}

export async function getLocalLlmModel(): Promise<string | null> {
  return invoke("get_local_llm_model");
}

export async function setLocalLlmModel(model: string | null): Promise<void> {
  return invoke("set_local_llm_model", { model });
}

export interface LocalLlmModel {
  name: string;
  size: number | null;
}

export async function listLocalLlmModels(endpoint: string): Promise<LocalLlmModel[]> {
  return invoke("list_local_llm_models", { endpoint });
}

export async function getEnhanceProvider(): Promise<string> {
  return invoke("get_enhance_provider");
}

export async function setEnhanceProvider(provider: string): Promise<void> {
  return invoke("set_enhance_provider", { provider });
}
