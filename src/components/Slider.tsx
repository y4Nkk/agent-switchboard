import { type CSSProperties } from "react";

const THUMB_SIZE_PX = 28;
const MIN_TICK_COUNT = 2;
const MAX_FULL_TICK_COUNT = 13;
const CONDENSED_TICK_COUNT = 7;
const FLOAT_COMPARISON_EPSILON = 1e-6;
const ENERGY_PARTICLE_COUNT = 9;

type SliderStyle = CSSProperties & {
  "--asb-slider-fill-width": string;
  "--asb-slider-thumb-left": string;
};

interface Props {
  value: number;
  min: number;
  max: number;
  step: number;
  ariaLabel: string;
  /** Human-readable current detent, announced instead of the raw number. */
  ariaValueText: string;
  disabled?: boolean;
  onValueChange: (value: number) => void;
}

/**
 * Discrete-detent slider with a pill track, energy-gradient fill clipped to
 * the value, glimmer particles, ticks and a white thumb over a native range
 * input. All visual values come from styles/tokens.css.
 */
export function Slider({ value, min, max, step, ariaLabel, ariaValueText, disabled = false, onValueChange }: Props) {
  assertSliderContract({ value, min, max, step });
  const percent = ((value - min) / (max - min)) * 100;
  const thumbOffset = (0.5 - percent / 100) * THUMB_SIZE_PX;
  const thumbLeft = `calc(${percent}% + ${thumbOffset}px)`;
  const fillWidth = percent >= 100 ? "100%" : thumbLeft;
  const style: SliderStyle = {
    "--asb-slider-fill-width": fillWidth,
    "--asb-slider-thumb-left": thumbLeft,
  };
  const stepCount = resolveTickCount({ min, max, step });

  return (
    <div className="asb-slider" data-disabled={disabled ? "true" : undefined} style={style}>
      <div className="asb-slider-track" aria-hidden="true">
        <div className="asb-slider-fill" />
        <span className="asb-slider-particles" aria-hidden="true">
          {Array.from({ length: ENERGY_PARTICLE_COUNT }, (_, index) => (
            <span key={index} className="asb-slider-particle" />
          ))}
        </span>
        <span className="asb-slider-ticks" aria-hidden="true">
          {Array.from({ length: stepCount }, (_, index) => {
            const tickPercent = (index / (stepCount - 1)) * 100;
            const active = tickPercent <= percent + FLOAT_COMPARISON_EPSILON;
            return (
              <span
                key={index}
                className="asb-slider-tick"
                data-active={active ? "true" : "false"}
                style={{ left: `${tickPercent}%` }}
              />
            );
          })}
        </span>
      </div>
      <span className="asb-slider-thumb" aria-hidden="true" />
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        aria-label={ariaLabel}
        aria-valuetext={ariaValueText}
        className="asb-slider-input"
        onChange={(event) => onValueChange(event.currentTarget.valueAsNumber)}
      />
    </div>
  );
}

/** Long ranges collapse to a fixed detent sample instead of dense tick rows. */
function resolveTickCount({ min, max, step }: Pick<Props, "min" | "max" | "step">): number {
  const stepCount = Math.floor((max - min) / step) + 1;
  if (stepCount > MAX_FULL_TICK_COUNT) return CONDENSED_TICK_COUNT;
  return Math.max(MIN_TICK_COUNT, stepCount);
}

function assertSliderContract({ value, min, max, step }: Pick<Props, "value" | "min" | "max" | "step">): void {
  const values = [value, min, max, step];
  if (!values.every(Number.isFinite)) throw new Error("Slider requires finite numeric values");
  if (max <= min) throw new Error("Slider max must be greater than min");
  if (step <= 0) throw new Error("Slider step must be greater than zero");
  if (value < min || value > max) throw new Error("Slider value must be within the configured range");
}
