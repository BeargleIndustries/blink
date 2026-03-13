export interface GenerationRequest {
  prompt: string;
  negative_prompt?: string;
  width?: number;
  height?: number;
  steps?: number;
  cfg_scale?: number;
  seed?: number;
  sampler?: string;
  input_image?: number[];
  strength?: number;
}

export interface GenerationProgress {
  step: number;
  total_steps: number;
  elapsed_secs: number;
}

export interface ModelInfo {
  id: string;
  name: string;
  description: string;
  architecture: string;
  size_bytes: number;
  vram_mb: number;
  license_name: string;
  license_url: string;
  commercial: boolean;
  downloaded: boolean;
  active: boolean;
  default_width: number;
  default_height: number;
  default_steps: number;
  default_cfg: number;
  default_sampler: string;
}

export interface DownloadProgress {
  model_id: string;
  downloaded_bytes: number;
  total_bytes: number;
  speed_bps: number;
}

export interface SystemInfo {
  compiled_backend: string;
  os: string;
  arch: string;
}

export interface AppSettings {
  theme: string;
  model_directory: string | null;
  default_save_directory: string | null;
  show_advanced: boolean;
  last_model: string | null;
  gallery_columns: number;
  auto_save_gallery: boolean;
}

export interface GalleryItem {
  id: string;
  filename: string;
  thumbnail_path: string;
  full_path: string;
  prompt: string;
  negative_prompt: string;
  model_id: string;
  model_name: string;
  width: number;
  height: number;
  steps: number;
  cfg_scale: number;
  seed: number;
  sampler: string;
  generated_at: string;
  generation_time_secs: number;
}