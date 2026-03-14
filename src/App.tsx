import { Component, createSignal, onMount, onCleanup, Show } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import type { ModelInfo, SystemInfo, GalleryItem, GenerationProgress, PerfSettings, FileDownloadProgress, LoraConfig } from "./lib/types";
import {
  generateImage,
  cancelGeneration,
  getModels,
  getSystemInfo,
  downloadModel,
  deleteModel,
  setActiveModel,
  getGallery,
  saveToGallery,
  loadGalleryImage,
  getPerfSettings,
  savePerfSettings,
  getHfToken,
  setHfToken,
} from "./lib/tauri-api";
import { getDefaultsForModel } from "./lib/defaults";

import PromptBar from "./components/PromptBar";
import ImageCanvas, { ImageCanvasAPI } from "./components/ImageCanvas";
import ProgressBar from "./components/ProgressBar";
import SettingsPanel from "./components/SettingsPanel";
import ModelSelector from "./components/ModelSelector";
import ModelBrowser from "./components/ModelBrowser";
import Gallery from "./components/Gallery";
import FirstRunWizard from "./components/FirstRunWizard";
import AboutDialog from "./components/AboutDialog";

const App: Component = () => {
  // UI state
  const [showBrowser, setShowBrowser] = createSignal(false);
  const [showAbout, setShowAbout] = createSignal(false);
  const [showWizard, setShowWizard] = createSignal(false);

  // Data state
  const [models, setModels] = createSignal<ModelInfo[]>([]);
  const [systemInfo, setSystemInfo] = createSignal<SystemInfo | null>(null);
  const [galleryItems, setGalleryItems] = createSignal<GalleryItem[]>([]);
  const [activeModelId, setActiveModelId] = createSignal<string | null>(null);

  // Generation state
  const [generating, setGenerating] = createSignal(false);
  const [currentStep, setCurrentStep] = createSignal(0);
  const [totalSteps, setTotalSteps] = createSignal(0);
  const [elapsed, setElapsed] = createSignal(0);
  const [generatedImage, setGeneratedImage] = createSignal<string | null>(null);
  const [previewImage, setPreviewImage] = createSignal<string | null>(null);
  const [inputImage, setInputImage] = createSignal<string | null>(null);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [modelLoading, setModelLoading] = createSignal(false);
  const [downloading, setDownloading] = createSignal<string | null>(null);
  const [downloadProgress, setDownloadProgress] = createSignal<{ modelId: string; fileRole: string; fileIndex: number; totalFiles: number } | null>(null);
  const [hfToken, setHfTokenState] = createSignal<string | null>(null);

  const [perfSettings, setPerfSettings] = createSignal<PerfSettings>({
    flash_attn: true,
    diffusion_flash_attn: true,
    enable_mmap: true,
    free_params_immediately: false,
    keep_clip_on_cpu: false,
    keep_vae_on_cpu: false,
    offload_params_to_cpu: false,
  });

  // Generation settings
  const [steps, setSteps] = createSignal(20);
  const [cfgScale, setCfgScale] = createSignal(7.0);
  const [seed, setSeed] = createSignal(-1);
  const [width, setWidth] = createSignal(512);
  const [height, setHeight] = createSignal(512);
  const [sampler, setSampler] = createSignal("euler_a");
  const [strength, setStrength] = createSignal(0.75);

  // Kontext / edit mode
  const [editMode, setEditMode] = createSignal(false);
  const [imgCfg, setImgCfg] = createSignal(1.5);

  // LoRA
  const [loras, setLoras] = createSignal<LoraConfig[]>([]);

  // ControlNet
  const [controlNetEnabled, setControlNetEnabled] = createSignal(false);
  const [controlStrength, setControlStrength] = createSignal(0.75);

  // Track last generation params for gallery save
  let lastPrompt = "";
  let lastNegativePrompt = "";

  // ImageCanvas API ref for mask access
  let imageCanvasApi: ImageCanvasAPI | undefined;

  const mode = () => {
    if (!inputImage()) return "txt2img";
    return editMode() ? "edit" : "img2img";
  };

  const activeModel = () => models().find((m) => m.active) ?? null;

  const applyModelDefaults = (modelId: string) => {
    const defaults = getDefaultsForModel(modelId);
    setSteps(defaults.steps);
    setCfgScale(defaults.cfg_scale);
    setWidth(defaults.width);
    setHeight(defaults.height);
    setSampler(defaults.sampler);
  };

  // Unlisten refs — populated async, cleaned up sync
  let unlisteners: Array<() => void> = [];
  onCleanup(() => unlisteners.forEach((fn) => fn()));

  onMount(async () => {
    try {
      const [loadedModels, sysInfo, gallery, perf, token] = await Promise.all([
        getModels(),
        getSystemInfo(),
        getGallery(),
        getPerfSettings(),
        getHfToken(),
      ]);

      setModels(loadedModels);
      setSystemInfo(sysInfo);
      setGalleryItems(gallery);
      setPerfSettings(perf);
      setHfTokenState(token);

      const active = loadedModels.find((m) => m.active);
      if (active) {
        setActiveModelId(active.id);
        applyModelDefaults(active.id);
      }

      const hasDownloaded = loadedModels.some((m) => m.downloaded);
      if (!hasDownloaded) {
        setShowWizard(true);
      }
    } catch (err) {
      console.error("Failed to initialize:", err);
    }

    unlisteners = await Promise.all([
      listen<GenerationProgress>("generation:progress", (event) => {
        setCurrentStep(event.payload.step);
        setTotalSteps(event.payload.total_steps);
        setElapsed(event.payload.elapsed_secs);
      }),

      listen<{ image_base64: string; width: number; height: number }>("generation:preview", (event) => {
        setPreviewImage(event.payload.image_base64);
      }),

      listen<{ image_base64: string; width: number; height: number; seed: number; generation_time_secs: number }>("generation:complete", async (event) => {
        setGenerating(false);
        setGeneratedImage(event.payload.image_base64);
        setPreviewImage(null);
        setInputImage(null);
        setCurrentStep(0);
        setTotalSteps(0);
        setElapsed(0);
        setErrorMessage(null);
        imageCanvasApi?.clearMask();

        const model = activeModel();
        try {
          const item = await saveToGallery({
            imageBase64: event.payload.image_base64,
            prompt: lastPrompt,
            negativePrompt: lastNegativePrompt,
            modelId: activeModelId() ?? "unknown",
            modelName: model?.name ?? "Unknown",
            width: event.payload.width,
            height: event.payload.height,
            steps: steps(),
            cfgScale: cfgScale(),
            seed: event.payload.seed,
            sampler: sampler(),
            generationTimeSecs: event.payload.generation_time_secs,
          });
          setGalleryItems((prev) => [item, ...prev]);
        } catch (err) {
          console.error("Failed to save to gallery:", err);
          getGallery().then(setGalleryItems).catch(console.error);
        }
      }),

      listen<{ message: string; recovery: string | null }>("generation:error", (event) => {
        setGenerating(false);
        setPreviewImage(null);
        setCurrentStep(0);
        setTotalSteps(0);
        setElapsed(0);
        setErrorMessage(event.payload.message);
        console.error("Generation error:", event.payload.message);
        setTimeout(() => setErrorMessage(null), 5000);
      }),

      listen("generation:cancelled", () => {
        setGenerating(false);
        setPreviewImage(null);
        setCurrentStep(0);
        setTotalSteps(0);
        setElapsed(0);
      }),

      listen<FileDownloadProgress>("model:download_file_start", (event) => {
        setDownloadProgress({
          modelId: event.payload.model_id,
          fileRole: event.payload.file_role,
          fileIndex: event.payload.file_index,
          totalFiles: event.payload.total_files,
        });
      }),

      listen<{ model_id: string; file_role: string }>("model:download_file_complete", (_event) => {
        // Next file_start will update progress; no action needed here
      }),

      listen("model:download_complete", async () => {
        setDownloading(null);
        setDownloadProgress(null);
        const updated = await getModels().catch(() => models());
        setModels(updated);
        setShowWizard(false);
      }),

      listen<string>("model:download_error", (event) => {
        setDownloading(null);
        setDownloadProgress(null);
        console.error("Model download error:", event.payload);
        setErrorMessage(`Download failed: ${event.payload}`);
        setTimeout(() => setErrorMessage(null), 5000);
      }),
    ]);
  });

  const handleGenerate = async (prompt: string, negativePrompt: string) => {
    if (!activeModelId()) return;
    lastPrompt = prompt;
    lastNegativePrompt = negativePrompt;
    setGenerating(true);
    setGeneratedImage(null);
    setPreviewImage(null);
    setCurrentStep(0);
    setTotalSteps(0);

    try {
      let inputBytes: number[] | undefined;
      if (inputImage()) {
        const base64 = inputImage()!.split(",")[1];
        const binary = atob(base64);
        inputBytes = Array.from(binary, (c) => c.charCodeAt(0));
      }

      const isEditMode = mode() === "edit";

      let maskBytes: number[] | undefined;
      if (inputBytes && !isEditMode) {
        const mask = imageCanvasApi?.getMask();
        if (mask) maskBytes = mask;
      }

      await generateImage({
        prompt,
        negative_prompt: negativePrompt || undefined,
        width: width(),
        height: height(),
        steps: steps(),
        cfg_scale: cfgScale(),
        seed: seed(),
        sampler: sampler(),
        input_image: isEditMode ? undefined : inputBytes,
        mask_image: isEditMode ? undefined : maskBytes,
        strength: (!isEditMode && inputBytes) ? strength() : undefined,
        ref_images: (isEditMode && inputBytes) ? [inputBytes] : undefined,
        img_cfg: isEditMode ? imgCfg() : undefined,
        loras: loras().length > 0 ? loras() : undefined,
        control_strength: controlNetEnabled() ? controlStrength() : undefined,
      });
    } catch (err) {
      setGenerating(false);
      console.error("Generate failed:", err);
    }
  };

  const handleCancel = async () => {
    try {
      await cancelGeneration();
      setGenerating(false);
      setCurrentStep(0);
      setTotalSteps(0);
      setElapsed(0);
    } catch (err) {
      console.error("Cancel failed:", err);
    }
  };

  const handleSelectModel = async (modelId: string) => {
    try {
      setModelLoading(true);
      setErrorMessage(null);
      await setActiveModel(modelId);
      const updated = await getModels();
      setModels(updated);
      setActiveModelId(modelId);
      applyModelDefaults(modelId);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setErrorMessage(`Failed to load model: ${msg}`);
      setTimeout(() => setErrorMessage(null), 10000);
      console.error("Failed to set active model:", err);
    } finally {
      setModelLoading(false);
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    try {
      setDownloading(modelId);
      await downloadModel(modelId);
    } catch (err) {
      setDownloading(null);
      console.error("Failed to download model:", err);
    }
  };

  const handleDeleteModel = async (modelId: string) => {
    try {
      await deleteModel(modelId);
      const updated = await getModels();
      setModels(updated);
      if (activeModelId() === modelId) {
        setActiveModelId(null);
      }
    } catch (err) {
      console.error("Failed to delete model:", err);
    }
  };

  const handleGallerySelect = async (item: GalleryItem) => {
    try {
      const base64 = await loadGalleryImage(item.id);
      setInputImage(null);
      setGeneratedImage(base64);
    } catch (err) {
      console.error("Failed to load gallery image:", err);
    }
  };

  const handleHfTokenChange = async (token: string | null) => {
    setHfTokenState(token);
    try {
      await setHfToken(token);
    } catch (err) {
      console.error("Failed to save HF token:", err);
    }
  };

  const handlePerfChange = async (settings: PerfSettings) => {
    setPerfSettings(settings);
    try {
      await savePerfSettings(settings);
    } catch (err) {
      console.error("Failed to save perf settings:", err);
    }
  };

  const handleImageDrop = (imageData: string) => {
    setInputImage(imageData);
    setGeneratedImage(null);
  };

  const handleClearImage = () => {
    setInputImage(null);
    setEditMode(false);
  };

  return (
    <div style={{
      display: "flex",
      "flex-direction": "column",
      height: "100vh",
      background: "var(--bg-primary)",
      color: "var(--text-primary)",
      overflow: "hidden",
    }}>
      <header style={{
        display: "flex",
        "align-items": "center",
        "justify-content": "space-between",
        padding: "8px 16px",
        background: "var(--bg-secondary)",
        "border-bottom": "1px solid var(--border)",
        "flex-shrink": "0",
      }}>
        <div style={{ display: "flex", "align-items": "center", gap: "12px" }}>
          <span style={{ "font-weight": "bold", "font-size": "16px" }}>
            Blink
          </span>
          <ModelSelector
            models={models()}
            activeModelId={activeModelId()}
            onSelectModel={handleSelectModel}
          />
          <Show when={systemInfo()}>
            {(() => {
              const info = systemInfo()!;
              const isCpu = info.compiled_backend.toLowerCase() === "cpu";
              return (
                <span
                  title={isCpu
                    ? "No GPU detected. Install CUDA Toolkit (NVIDIA) or use Metal (Mac) for faster generation."
                    : `Using ${info.compiled_backend} acceleration`}
                  style={{
                    padding: "2px 8px",
                    "border-radius": "var(--radius-pill)",
                    "font-size": "11px",
                    "font-weight": "600",
                    background: isCpu ? "rgba(245, 158, 11, 0.1)" : "rgba(34, 197, 94, 0.1)",
                    color: isCpu ? "var(--warning)" : "var(--success)",
                    border: `1px solid ${isCpu ? "rgba(245, 158, 11, 0.2)" : "rgba(34, 197, 94, 0.2)"}`,
                    cursor: "default",
                  }}
                >
                  {isCpu ? "CPU" : info.compiled_backend}
                </span>
              );
            })()}
          </Show>
          <button
            onClick={() => setShowBrowser(true)}
            style={{
              padding: "4px 12px",
              background: "var(--bg-tertiary)",
              border: "1px solid var(--border)",
              "border-radius": "var(--radius)",
              color: "var(--text-primary)",
              cursor: "pointer",
              "font-size": "13px",
            }}
          >
            Models
          </button>
        </div>
        <div style={{ display: "flex", gap: "8px" }}>
          <button
            onClick={() => setShowAbout(true)}
            style={{
              padding: "4px 8px",
              background: "none",
              border: "none",
              color: "var(--text-secondary)",
              cursor: "pointer",
              "font-size": "13px",
            }}
          >
            About
          </button>
        </div>
      </header>

      <main style={{
        flex: "1",
        display: "flex",
        "flex-direction": "column",
        "align-items": "center",
        "justify-content": "center",
        padding: "16px 24px",
        gap: "12px",
        overflow: "auto",
      }}>
        <ImageCanvas
          imageData={generatedImage()}
          generating={generating()}
          onImageDrop={handleImageDrop}
          onClearImage={handleClearImage}
          inputImage={inputImage()}
          previewImage={previewImage()}
          ref={(api) => { imageCanvasApi = api; }}
        />

        <Show when={inputImage()}>
          <div style={{ display: "flex", "align-items": "center", gap: "8px" }}>
            <button
              onClick={() => setEditMode(false)}
              style={{
                padding: "4px 12px",
                background: !editMode() ? "var(--accent)" : "var(--bg-tertiary)",
                border: `1px solid ${!editMode() ? "var(--accent)" : "var(--border)"}`,
                "border-radius": "var(--radius)",
                color: !editMode() ? "#fff" : "var(--text-secondary)",
                cursor: "pointer",
                "font-size": "12px",
                transition: "all 0.15s",
              }}
            >
              img2img
            </button>
            <button
              onClick={() => setEditMode(true)}
              style={{
                padding: "4px 12px",
                background: editMode() ? "var(--accent)" : "var(--bg-tertiary)",
                border: `1px solid ${editMode() ? "var(--accent)" : "var(--border)"}`,
                "border-radius": "var(--radius)",
                color: editMode() ? "#fff" : "var(--text-secondary)",
                cursor: "pointer",
                "font-size": "12px",
                transition: "all 0.15s",
              }}
            >
              Edit Mode
            </button>
            <Show when={editMode()}>
              <span style={{ "font-size": "11px", color: "var(--text-muted)", opacity: "0.6" }}>
                Kontext — describe the edit
              </span>
            </Show>
          </div>
        </Show>

        <Show when={inputImage() && editMode()}>
          <div style={{
            display: "flex",
            "align-items": "center",
            gap: "8px",
            width: "100%",
            "max-width": "700px",
            padding: "8px 12px",
            background: "var(--bg-secondary)",
            "border-radius": "var(--radius)",
          }}>
            <span style={{ "font-size": "12px", color: "var(--text-secondary)", "min-width": "110px" }}>
              Image Conditioning
            </span>
            <input
              type="range"
              min="0.5"
              max="3.0"
              step="0.05"
              value={imgCfg()}
              onInput={(e) => setImgCfg(parseFloat(e.currentTarget.value))}
              style={{ flex: "1" }}
            />
            <span style={{ "font-size": "13px", "min-width": "35px" }}>
              {imgCfg().toFixed(2)}
            </span>
          </div>
        </Show>

        <ProgressBar
          step={currentStep()}
          totalSteps={totalSteps()}
          visible={generating()}
          elapsed={elapsed()}
        />

        <Show when={errorMessage()}>
          <div style={{
            width: "512px",
            "max-width": "100%",
            padding: "8px 12px",
            background: "rgba(239, 68, 68, 0.12)",
            border: "1px solid var(--error)",
            "border-radius": "var(--radius)",
            color: "var(--error)",
            "font-size": "13px",
          }}>
            {errorMessage()}
          </div>
        </Show>

        <PromptBar
          onGenerate={handleGenerate}
          onCancel={handleCancel}
          generating={generating()}
          modelLoading={modelLoading()}
          modelReady={!!activeModelId() && !modelLoading()}
          mode={(mode() === "edit" ? "img2img" : mode()) as "txt2img" | "img2img"}
        />

        <SettingsPanel
          steps={steps()}
          cfgScale={cfgScale()}
          seed={seed()}
          width={width()}
          height={height()}
          sampler={sampler()}
          onStepsChange={setSteps}
          onCfgChange={setCfgScale}
          onSeedChange={setSeed}
          onWidthChange={setWidth}
          onHeightChange={setHeight}
          onSamplerChange={setSampler}
          strength={strength()}
          onStrengthChange={setStrength}
          showStrength={mode() === "img2img"}
          perfSettings={perfSettings()}
          onPerfChange={handlePerfChange}
          hfToken={hfToken()}
          onHfTokenChange={handleHfTokenChange}
          controlNetEnabled={controlNetEnabled()}
          onControlNetChange={setControlNetEnabled}
          controlStrength={controlStrength()}
          onControlStrengthChange={setControlStrength}
          showControlNet={mode() === "img2img"}
          loras={loras()}
          onLorasChange={setLoras}
        />
      </main>

      <footer style={{
        padding: "8px 16px",
        background: "var(--bg-secondary)",
        "border-top": "1px solid var(--border)",
        "min-height": "72px",
        "flex-shrink": "0",
      }}>
        <Gallery
          items={galleryItems()}
          onSelect={handleGallerySelect}
          onDelete={(id) => {
            setGalleryItems((prev) => prev.filter((i) => i.id !== id));
          }}
        />
      </footer>

      <Show when={showWizard()}>
        <FirstRunWizard
          models={models()}
          systemInfo={systemInfo()}
          downloading={downloading()}
          onDownload={handleDownloadModel}
          onSkip={() => setShowWizard(false)}
        />
      </Show>

      <Show when={showBrowser()}>
        <ModelBrowser
          models={models()}
          downloading={downloading()}
          downloadProgress={downloadProgress()}
          onDownload={handleDownloadModel}
          onDelete={handleDeleteModel}
          onSelect={(id) => {
            handleSelectModel(id);
            setShowBrowser(false);
          }}
          onClose={() => setShowBrowser(false)}
          vramTotalMb={systemInfo()?.vram_total_mb}
        />
      </Show>

      <Show when={showAbout()}>
        <AboutDialog onClose={() => setShowAbout(false)} />
      </Show>
    </div>
  );
};

export default App;
