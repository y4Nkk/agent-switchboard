import { useEffect, useRef, type RefObject } from "react";

export type MatrixStarlightVariant = "route-active" | "route-idle";

type Rgb = readonly [red: number, green: number, blue: number];

type MatrixStarlightTone = {
  baseColor: Rgb;
  flareColor: Rgb;
  baseOpacity: number;
  flareOpacity: number;
};

/* Color channels are owned by tokens.css (--asb-route-particle-*-rgb);
   this table only tunes effect intensity. */
const TONE_OPACITY = {
  "route-active": { baseOpacity: 0.5, flareOpacity: 0.96 },
  "route-idle": { baseOpacity: 0.3, flareOpacity: 0.62 },
} as const;

const TONE_TOKENS = {
  "route-active": {
    base: "--asb-route-particle-base-rgb",
    flare: "--asb-route-particle-flare-rgb",
  },
  "route-idle": {
    base: "--asb-route-particle-idle-base-rgb",
    flare: "--asb-route-particle-idle-flare-rgb",
  },
} as const;

const MATRIX_STARLIGHT = {
  maximumPoints: 2800,
  minimumCellSize: 10,
  maximumPixelRatio: 1.5,
  frameInterval: 1000 / 30,
  topFadeStart: 0.2,
  topFadeEnd: 0.52,
  contentSafeZoneWidth: 0.58,
  contentSafeZoneHeight: 0.62,
  contentSafeZoneOpacity: 0.42,
  flareNoiseThreshold: 0.95,
  flareWaveThreshold: 0.7,
  phaseSpeed: 0.00065,
} as const;

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

function smoothstep(start: number, end: number, value: number): number {
  const progress = clamp((value - start) / (end - start), 0, 1);
  return progress * progress * (3 - 2 * progress);
}

function latticeNoise(column: number, row: number): number {
  const value = Math.sin(column * 12.9898 + row * 78.233) * 43758.5453;
  return value - Math.floor(value);
}

function toRgb([red, green, blue]: Rgb): string {
  return `rgb(${red} ${green} ${blue})`;
}

function readRgbToken(style: CSSStyleDeclaration, token: string): Rgb | null {
  const match = /^(\d{1,3})\s+(\d{1,3})\s+(\d{1,3})$/.exec(
    style.getPropertyValue(token).trim(),
  );
  return match
    ? [Number(match[1]), Number(match[2]), Number(match[3])]
    : null;
}

type MatrixDrawInput = {
  context: CanvasRenderingContext2D;
  height: number;
  timestamp: number;
  tone: MatrixStarlightTone;
  width: number;
};

type MatrixRenderState = {
  baseColor: string;
  cellSize: number;
  columns: number;
  flareColor: string;
  phase: number;
  rows: number;
  tone: MatrixStarlightTone;
};

type MatrixPoint = {
  column: number;
  row: number;
  horizontalPosition: number;
  verticalPosition: number;
  verticalFade: number;
};

function createMatrixRenderState({
  height,
  timestamp,
  tone,
  width,
}: Omit<MatrixDrawInput, "context">): MatrixRenderState {
  const cellSize = Math.max(
    MATRIX_STARLIGHT.minimumCellSize,
    Math.sqrt((width * height) / MATRIX_STARLIGHT.maximumPoints),
  );

  return {
    baseColor: toRgb(tone.baseColor),
    cellSize,
    columns: Math.ceil(width / cellSize),
    flareColor: toRgb(tone.flareColor),
    phase: timestamp * MATRIX_STARLIGHT.phaseSpeed,
    rows: Math.ceil(height / cellSize),
    tone,
  };
}

function drawMatrixPoint(
  context: CanvasRenderingContext2D,
  point: MatrixPoint,
  state: MatrixRenderState,
): void {
  const { column, horizontalPosition, row, verticalFade, verticalPosition } =
    point;
  const { baseColor, cellSize, flareColor, phase, tone } = state;
  const noise = latticeNoise(column, row);
  const wave = 0.5 + 0.5 * Math.sin(column * 0.46 - row * 0.22 + phase);
  const shimmer =
    0.5 + 0.5 * Math.sin(column * 0.19 + row * 0.54 - phase * 1.4);
  const inContentSafeZone =
    horizontalPosition < MATRIX_STARLIGHT.contentSafeZoneWidth &&
    verticalPosition < MATRIX_STARLIGHT.contentSafeZoneHeight;
  const safeZoneOpacity = inContentSafeZone
    ? MATRIX_STARLIGHT.contentSafeZoneOpacity
    : 1;
  const opacity =
    tone.baseOpacity *
    verticalFade *
    safeZoneOpacity *
    (0.22 + noise * 0.2 + wave * 0.34 + shimmer * 0.24);

  if (opacity < 0.04) return;

  const x = column * cellSize + cellSize / 2;
  const y = row * cellSize + cellSize / 2;
  const size = noise > 0.9 ? 2 : 1;

  context.globalAlpha = opacity;
  context.fillRect(x, y, size, size);

  if (
    noise <= MATRIX_STARLIGHT.flareNoiseThreshold ||
    wave <= MATRIX_STARLIGHT.flareWaveThreshold
  )
    return;

  context.fillStyle = flareColor;
  context.globalAlpha =
    tone.flareOpacity * verticalFade * safeZoneOpacity * shimmer;
  context.fillRect(x - 0.5, y - 0.5, 2, 2);
  context.fillStyle = baseColor;
}

function drawMatrixStarlight({
  context,
  height,
  timestamp,
  tone,
  width,
}: MatrixDrawInput): void {
  const state = createMatrixRenderState({ height, timestamp, tone, width });
  const firstRow = Math.floor(state.rows * MATRIX_STARLIGHT.topFadeStart);

  context.clearRect(0, 0, width, height);
  context.fillStyle = state.baseColor;

  for (let row = firstRow; row <= state.rows; row += 1) {
    const verticalPosition = row / state.rows;
    const verticalFade = smoothstep(
      MATRIX_STARLIGHT.topFadeStart,
      MATRIX_STARLIGHT.topFadeEnd,
      verticalPosition,
    );

    for (let column = 0; column <= state.columns; column += 1) {
      drawMatrixPoint(
        context,
        {
          column,
          row,
          horizontalPosition: column / state.columns,
          verticalPosition,
          verticalFade,
        },
        state,
      );
    }
  }

  context.globalAlpha = 1;
}

type MatrixCanvasSetup = {
  canvas: HTMLCanvasElement;
  context: CanvasRenderingContext2D;
  motionQuery: MediaQueryList;
  tone: MatrixStarlightTone;
};

type MatrixCanvasDimensions = {
  height: number;
  pixelRatio: number;
  width: number;
};

function getMatrixCanvasDimensions(
  canvas: HTMLCanvasElement,
): MatrixCanvasDimensions {
  const bounds = canvas.getBoundingClientRect();
  return {
    height: Math.round(bounds.height),
    pixelRatio: Math.min(
      window.devicePixelRatio || 1,
      MATRIX_STARLIGHT.maximumPixelRatio,
    ),
    width: Math.round(bounds.width),
  };
}

function resolveTone(
  variant: MatrixStarlightVariant,
  canvas: HTMLCanvasElement,
): MatrixStarlightTone | null {
  const style = window.getComputedStyle(canvas);
  const tokens = TONE_TOKENS[variant];
  const baseColor = readRgbToken(style, tokens.base);
  const flareColor = readRgbToken(style, tokens.flare);
  if (!baseColor || !flareColor) return null;
  return { baseColor, flareColor, ...TONE_OPACITY[variant] };
}

function setupMatrixStarlight({
  canvas,
  context,
  motionQuery,
  tone,
}: MatrixCanvasSetup): () => void {
  let dimensions = getMatrixCanvasDimensions(canvas);
  let frameId = 0;
  let lastFrameAt = 0;

  const paint = (timestamp: number) => {
    if (!dimensions.width || !dimensions.height) return;
    context.setTransform(
      dimensions.pixelRatio,
      0,
      0,
      dimensions.pixelRatio,
      0,
      0,
    );
    drawMatrixStarlight({ context, timestamp, tone, ...dimensions });
  };
  const resize = () => {
    dimensions = getMatrixCanvasDimensions(canvas);
    canvas.width = Math.round(dimensions.width * dimensions.pixelRatio);
    canvas.height = Math.round(dimensions.height * dimensions.pixelRatio);
    paint(motionQuery.matches ? 0 : performance.now());
  };
  const animate = (timestamp: number) => {
    if (timestamp - lastFrameAt >= MATRIX_STARLIGHT.frameInterval) {
      paint(timestamp);
      lastFrameAt = timestamp;
    }
    frameId = window.requestAnimationFrame(animate);
  };
  const syncMotion = () => {
    window.cancelAnimationFrame(frameId);
    if (motionQuery.matches) {
      paint(0);
      return;
    }
    lastFrameAt = 0;
    frameId = window.requestAnimationFrame(animate);
  };
  const observer = new ResizeObserver(resize);

  observer.observe(canvas);
  motionQuery.addEventListener("change", syncMotion);
  resize();
  syncMotion();

  return () => {
    window.cancelAnimationFrame(frameId);
    observer.disconnect();
    motionQuery.removeEventListener("change", syncMotion);
  };
}

function useMatrixStarlightCanvas(
  canvasRef: RefObject<HTMLCanvasElement | null>,
  variant: MatrixStarlightVariant,
): void {
  useEffect(() => {
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;

    const tone = resolveTone(variant, canvas);
    if (!tone) return;

    return setupMatrixStarlight({
      canvas,
      context,
      motionQuery: window.matchMedia("(prefers-reduced-motion: reduce)"),
      tone,
    });
  }, [canvasRef, variant]);
}

/** Starlight lattice behind route cards; content readability comes from the
    in-canvas top fade and top-left safe zone, not a CSS mask. */
export function MatrixStarlightCanvas({
  variant,
}: {
  variant: MatrixStarlightVariant;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useMatrixStarlightCanvas(canvasRef, variant);

  return (
    <canvas
      ref={canvasRef}
      className="asb-card-particle-canvas"
      aria-hidden="true"
    />
  );
}
