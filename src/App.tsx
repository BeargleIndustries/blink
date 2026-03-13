import { Component, createSignal, onMount, onCleanup, Show } from "solid-js";
import { listen } from "@tauri-apps/api/event";
import type { ModelInfo, SystemInfo, GalleryItem, GenerationProgress } from "./lib/types";
import {
  generateImage,
  cancelGeneration,
  getModels,
  getSystemInfo,
  downloadModel,
  deleteModel,
  setActiveModel,
  getGallery,
} from "./lib/tauri-api";
import { getDefaultsForModel } from "./lib/defaults";

import PromptBar from "./components/PromptBar";
import ImageCanvas from "./components/ImageCanvas";
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
  const [inputImage, setInputImage] = createSignal<string | null>(null);
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);

  // Generation settings
  const [steps, setSteps] = createSignal(20);
  const [cfgScale, setCfgScale] = createSignal(7.0);
  const [seed, setSeed] = createSignal(-1);
  const [width, setWidth] = createSignal(512);
  const [height, setHeight] = createSignal(512);
  const [sampler, setSampler] = createSignal("euler_a");
  const [strength, setStrength] = createSignal(0.75);

  const mode = () => (inputImage() ? "img2img" : "txt2img");

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
    // Load data
    try {
      const [loadedModels, sysInfo, gallery] = await Promise.all([
        getModels(),
        getSystemInfo(),
        getGallery(),
      ]);

      setModels(loadedModels);
      setSystemInfo(sysInfo);
      setGalleryItems(gallery);

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

    // Tauri event listeners — store unlisten fns for cleanup
    unlisteners = await Promise.all([
      listen<GenerationProgress>("generation:progress", (event) => {
        setCurrentStep(event.payload.step);
        setTotalSteps(event.payload.total_steps);
        setElapsed(event.payload.elapsed_secs);
      }),

      listen<{ image_base64: string; width: number; height: number; seed: number; generation_time_secs: number }>("generation:complete", (event) => {
        setGenerating(false);
        setGeneratedImage(event.payload.image_base64);
        setCurrentStep(0);
        setTotalSteps(0);
        setElapsed(0);
        setErrorMessage(null);
        getGallery().then(setGalleryItems).catch(console.error);
      }),

      listen<{ message: string; recovery: string | null }>("generation:error", (event) => {
        setGenerating(false);
        setCurrentStep(0);
        setTotalSteps(0);
        setElapsed(0);
        setErrorMessage(event.payload.message);
        console.error("Generation error:", event.payload.message);
        // Auto-clear error after 5s
        setTimeout(() => setErrorMessage(null), 5000);
      }),

      listen("generation:cancelled", () => {
        setGenerating(false);
        setCurrentStep(0);
        setTotalSteps(0);
        setElapsed(0);
      }),

      listen("model:download_complete", async () => {
        const updated = await getModels().catch(() => models());
        setModels(updated);
        setShowWizard(false);
      }),

      listen<string>("model:download_error", (event) => {
        console.error("Model download error:", event.payload);
        setErrorMessage(`Download failed: ${event.payload}`);
        setTimeout(() => setErrorMessage(null), 5000);
      }),
    ]);
  });

  const handleGenerate = async (prompt: string, negativePrompt: string) => {
    if (!activeModelId()) return;
    setGenerating(true);
    setGeneratedImage(null);
    setCurrentStep(0);
    setTotalSteps(0);

    try {
      // Convert inputImage dataURL to bytes if present
      let inputBytes: number[] | undefined;
      if (inputImage()) {
        const base64 = inputImage()!.split(",")[1];
        const binary = atob(base64);
        inputBytes = Array.from(binary, (c) => c.charCodeAt(0));
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
        input_image: inputBytes,
        strength: inputBytes ? strength() : undefined,
      });
    } catch (err) {
      setGenerating(false);
      console.error("Generate failed:", err);
    }
  };

  const handleCancel = async () => {
    try {
      await cancelGeneration();
    } catch (err) {
      console.error("Cancel failed:", err);
    }
  };

  const handleSelectModel = async (modelId: string) => {
    try {
      await setActiveModel(modelId);
      const updated = await getModels();
      setModels(updated);
      setActiveModelId(modelId);
      applyModelDefaults(modelId);
    } catch (err) {
      console.error("Failed to set active model:", err);
    }
  };

  const handleDownloadModel = async (modelId: string) => {
    try {
      await downloadModel(modelId);
      const updated = await getModels();
      setModels(updated);
      setShowWizard(false);
    } catch (err) {
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

  const handleGallerySelect = (item: GalleryItem) => {
    // Use asset protocol to load from disk path
    setGeneratedImage(`asset://localhost/${item.filename}`);
  };

  const handleImageDrop = (imageData: string) => {
    setInputImage(imageData);
    setGeneratedImage(null);
  };

  const handleClearImage = () => {
    setInputImage(null);
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
      {/* Header */}
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

      {/* Main Content */}
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
        />

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
            background: "rgba(244, 67, 54, 0.12)",
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
          mode={mode()}
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
        />
      </main>

      {/* Gallery Footer */}
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

      {/* Modals */}
      <Show when={showWizard()}>
        <FirstRunWizard
          models={models()}
          systemInfo={systemInfo()}
          onDownload={handleDownloadModel}
          onSkip={() => setShowWizard(false)}
        />
      </Show>

      <Show when={showBrowser()}>
        <ModelBrowser
          models={models()}
          onDownload={handleDownloadModel}
          onDelete={handleDeleteModel}
          onSelect={(id) => {
            handleSelectModel(id);
            setShowBrowser(false);
          }}
          onClose={() => setShowBrowser(false)}
        />
      </Show>

      <Show when={showAbout()}>
        <AboutDialog onClose={() => setShowAbout(false)} />
      </Show>
    </div>
  );
};

export default App;
