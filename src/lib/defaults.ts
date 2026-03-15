export interface ModelDefaults {
  width: number;
  height: number;
  steps: number;
  cfg_scale: number;
  sampler: string;
}

export const MODEL_DEFAULTS: Record<string, ModelDefaults> = {
  "sd15-q5": {
    width: 512,
    height: 512,
    steps: 20,
    cfg_scale: 7.0,
    sampler: "euler_a",
  },
  "sdxl-q4": {
    width: 1024,
    height: 1024,
    steps: 25,
    cfg_scale: 7.0,
    sampler: "euler_a",
  },
  "flux-schnell-q4": {
    width: 1024,
    height: 1024,
    steps: 4,
    cfg_scale: 1.0,
    sampler: "euler",
  },
  "sd15-q8": {
    width: 512,
    height: 512,
    steps: 20,
    cfg_scale: 7.0,
    sampler: "euler_a",
  },
  "flux-dev-q4": {
    width: 1024,
    height: 1024,
    steps: 25,
    cfg_scale: 3.5,
    sampler: "euler",
  },
  "z-image-turbo": {
    width: 512,
    height: 1024,
    steps: 4,
    cfg_scale: 1.0,
    sampler: "euler",
  },
  "z-image-turbo-q8": {
    width: 512,
    height: 1024,
    steps: 4,
    cfg_scale: 1.0,
    sampler: "euler",
  },
  "flux-schnell-q8": {
    width: 1024,
    height: 1024,
    steps: 4,
    cfg_scale: 1.0,
    sampler: "euler",
  },
  "flux-kontext-dev-q4": {
    width: 1024,
    height: 1024,
    steps: 35,
    cfg_scale: 3.5,
    sampler: "euler",
  },
  "flux-kontext-dev-q8": {
    width: 1024,
    height: 1024,
    steps: 35,
    cfg_scale: 3.5,
    sampler: "euler",
  },
};

export function getDefaultsForModel(modelId: string, modelInfo?: { default_width: number; default_height: number; default_steps: number; default_cfg: number; default_sampler: string }): ModelDefaults {
  if (MODEL_DEFAULTS[modelId]) {
    return MODEL_DEFAULTS[modelId];
  }
  if (modelInfo) {
    return {
      width: modelInfo.default_width,
      height: modelInfo.default_height,
      steps: modelInfo.default_steps,
      cfg_scale: modelInfo.default_cfg,
      sampler: modelInfo.default_sampler,
    };
  }
  return MODEL_DEFAULTS["sd15-q5"];
}
