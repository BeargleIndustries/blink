import { Component, createEffect, onCleanup, onMount } from "solid-js";

export interface MaskCanvasAPI {
  exportMask: () => number[] | null;
  clearMask: () => void;
}

interface MaskCanvasProps {
  containerWidth: number;
  containerHeight: number;
  imgNaturalWidth: number;
  imgNaturalHeight: number;
  brushSize: number;
  isEraser: boolean;
  isActive: boolean;
  onMaskChange: (hasMask: boolean) => void;
  ref?: (api: MaskCanvasAPI) => void;
}

const MaskCanvas: Component<MaskCanvasProps> = (props) => {
  let canvasRef: HTMLCanvasElement | undefined;
  let cursorCanvasRef: HTMLCanvasElement | undefined;

  let isDrawing = false;
  let lastX = 0;
  let lastY = 0;
  let hasMask = false;

  // Compute letterbox rect
  const getRenderedRect = () => {
    const scale = Math.min(
      props.containerWidth / props.imgNaturalWidth,
      props.containerHeight / props.imgNaturalHeight
    );
    const renderedW = props.imgNaturalWidth * scale;
    const renderedH = props.imgNaturalHeight * scale;
    const offsetX = (props.containerWidth - renderedW) / 2;
    const offsetY = (props.containerHeight - renderedH) / 2;
    return { renderedW, renderedH, offsetX, offsetY };
  };

  // Convert pointer page coords to canvas-space coords
  const toCanvasCoords = (pageX: number, pageY: number) => {
    if (!canvasRef) return { x: 0, y: 0 };
    const rect = canvasRef.getBoundingClientRect();
    const x = (pageX - rect.left) * (canvasRef.width / rect.width);
    const y = (pageY - rect.top) * (canvasRef.height / rect.height);
    return { x, y };
  };

  const getCanvasScale = () => {
    if (!canvasRef) return 1;
    const rect = canvasRef.getBoundingClientRect();
    return rect.width > 0 ? canvasRef.width / rect.width : 1;
  };

  const drawCircle = (ctx: CanvasRenderingContext2D, x: number, y: number) => {
    const scaledRadius = (props.brushSize / 2) * getCanvasScale();
    ctx.beginPath();
    ctx.arc(x, y, scaledRadius, 0, Math.PI * 2);
    ctx.fill();
  };

  const interpolateAndDraw = (
    ctx: CanvasRenderingContext2D,
    x0: number,
    y0: number,
    x1: number,
    y1: number
  ) => {
    const scaledBrush = props.brushSize * getCanvasScale();
    const dist = Math.hypot(x1 - x0, y1 - y0);
    const steps = Math.max(1, Math.ceil(dist / (scaledBrush / 4)));
    for (let i = 0; i <= steps; i++) {
      const t = i / steps;
      const x = x0 + (x1 - x0) * t;
      const y = y0 + (y1 - y0) * t;
      drawCircle(ctx, x, y);
    }
  };

  const setDrawingMode = (ctx: CanvasRenderingContext2D) => {
    if (props.isEraser) {
      ctx.globalCompositeOperation = "destination-out";
      ctx.fillStyle = "rgba(255,255,255,1)";
    } else {
      ctx.globalCompositeOperation = "source-over";
      ctx.fillStyle = "rgba(255, 0, 0, 0.4)";
    }
  };

  const drawCursor = (x: number, y: number) => {
    if (!cursorCanvasRef) return;
    const ctx = cursorCanvasRef.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, cursorCanvasRef.width, cursorCanvasRef.height);
    const r = (props.brushSize / 2) * getCanvasScale();
    ctx.beginPath();
    ctx.arc(x, y, r, 0, Math.PI * 2);
    ctx.strokeStyle = props.isEraser
      ? "rgba(100,200,255,0.9)"
      : "rgba(255,100,100,0.9)";
    ctx.lineWidth = 1.5;
    ctx.stroke();
    // crosshair dot
    ctx.beginPath();
    ctx.arc(x, y, 1.5, 0, Math.PI * 2);
    ctx.fillStyle = ctx.strokeStyle;
    ctx.fill();
  };

  const clearCursor = () => {
    if (!cursorCanvasRef) return;
    const ctx = cursorCanvasRef.getContext("2d");
    ctx?.clearRect(0, 0, cursorCanvasRef.width, cursorCanvasRef.height);
  };

  const handlePointerDown = (e: PointerEvent) => {
    if (!props.isActive || !canvasRef) return;
    e.preventDefault();
    canvasRef.setPointerCapture(e.pointerId);
    isDrawing = true;
    const { x, y } = toCanvasCoords(e.clientX, e.clientY);
    lastX = x;
    lastY = y;
    const ctx = canvasRef.getContext("2d");
    if (!ctx) return;
    setDrawingMode(ctx);
    drawCircle(ctx, x, y);
    hasMask = true;
    props.onMaskChange(true);
  };

  const handlePointerMove = (e: PointerEvent) => {
    if (!canvasRef) return;
    const { x, y } = toCanvasCoords(e.clientX, e.clientY);
    drawCursor(x, y);
    if (!isDrawing || !props.isActive) return;
    e.preventDefault();
    const ctx = canvasRef.getContext("2d");
    if (!ctx) return;
    setDrawingMode(ctx);
    interpolateAndDraw(ctx, lastX, lastY, x, y);
    lastX = x;
    lastY = y;
  };

  const handlePointerUp = (e: PointerEvent) => {
    if (!isDrawing) return;
    isDrawing = false;
    if (canvasRef) canvasRef.releasePointerCapture(e.pointerId);
  };

  const handlePointerLeave = () => {
    clearCursor();
    isDrawing = false;
  };

  const clearMask = () => {
    if (!canvasRef) return;
    const ctx = canvasRef.getContext("2d");
    ctx?.clearRect(0, 0, canvasRef.width, canvasRef.height);
    hasMask = false;
    props.onMaskChange(false);
  };

  const exportMask = (): number[] | null => {
    if (!canvasRef || !hasMask) return null;

    const src = canvasRef.getContext("2d");
    if (!src) return null;

    const imageData = src.getImageData(0, 0, canvasRef.width, canvasRef.height);
    const { data } = imageData;

    // Check if anything was actually painted
    let anyPainted = false;
    for (let i = 3; i < data.length; i += 4) {
      if (data[i] > 0) { anyPainted = true; break; }
    }
    if (!anyPainted) return null;

    // Build a white/black mask at native resolution
    const offscreen = document.createElement("canvas");
    offscreen.width = canvasRef.width;
    offscreen.height = canvasRef.height;
    const offCtx = offscreen.getContext("2d");
    if (!offCtx) return null;

    const maskData = offCtx.createImageData(canvasRef.width, canvasRef.height);
    for (let i = 0; i < data.length; i += 4) {
      const alpha = data[i + 3];
      const val = alpha > 0 ? 255 : 0;
      maskData.data[i]     = val; // R
      maskData.data[i + 1] = val; // G
      maskData.data[i + 2] = val; // B
      maskData.data[i + 3] = 255; // A fully opaque
    }
    offCtx.putImageData(maskData, 0, 0);

    const dataUrl = offscreen.toDataURL("image/png");
    const base64 = dataUrl.replace(/^data:image\/png;base64,/, "");
    const binary = atob(base64);
    return Array.from(binary, (c) => c.charCodeAt(0));
  };

  onMount(() => {
    if (canvasRef) {
      canvasRef.width = props.imgNaturalWidth;
      canvasRef.height = props.imgNaturalHeight;
    }
    if (cursorCanvasRef) {
      cursorCanvasRef.width = props.imgNaturalWidth;
      cursorCanvasRef.height = props.imgNaturalHeight;
    }

    // Expose API to parent
    if (props.ref) {
      props.ref({ exportMask, clearMask });
    }
  });

  // Resize internal canvas resolution if natural dimensions change
  createEffect(() => {
    const w = props.imgNaturalWidth;
    const h = props.imgNaturalHeight;
    if (canvasRef && (canvasRef.width !== w || canvasRef.height !== h)) {
      canvasRef.width = w;
      canvasRef.height = h;
    }
    if (cursorCanvasRef && (cursorCanvasRef.width !== w || cursorCanvasRef.height !== h)) {
      cursorCanvasRef.width = w;
      cursorCanvasRef.height = h;
    }
  });

  onCleanup(() => {
    isDrawing = false;
  });

  const rect = () => getRenderedRect();

  return (
    <>
      {/* Drawing canvas */}
      <canvas
        ref={el => { canvasRef = el; }}
        style={{
          position: "absolute",
          left: `${rect().offsetX}px`,
          top: `${rect().offsetY}px`,
          width: `${rect().renderedW}px`,
          height: `${rect().renderedH}px`,
          "touch-action": "none",
          cursor: "none",
          opacity: props.isActive ? "1" : "0.6",
          "pointer-events": props.isActive ? "auto" : "none",
        }}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerLeave={handlePointerLeave}
      />
      {/* Cursor overlay canvas — separate so cursor doesn't bake into mask */}
      <canvas
        ref={el => { cursorCanvasRef = el; }}
        style={{
          position: "absolute",
          left: `${rect().offsetX}px`,
          top: `${rect().offsetY}px`,
          width: `${rect().renderedW}px`,
          height: `${rect().renderedH}px`,
          "touch-action": "none",
          cursor: "none",
          "pointer-events": "none",
        }}
      />
    </>
  );
};

export default MaskCanvas;
