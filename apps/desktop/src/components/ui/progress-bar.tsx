import type { CSSProperties } from "react";
import { cn } from "@/lib/utils";

type ProgressBarTone = "primary" | "success" | "warning" | "destructive" | "muted";

interface ProgressBarPropsBase {
  className?: string;
  trackClassName?: string;
}

interface ProgressBarToneProps extends ProgressBarPropsBase {
  /** 0-100 fill percentage. Values outside this range are clamped. */
  value: number;
  tone?: ProgressBarTone;
  percent?: undefined;
  color?: undefined;
}

interface ProgressBarLegacyProps extends ProgressBarPropsBase {
  /** 0-100 fill percentage. Values outside this range are clamped. */
  percent: number;
  /** CSS color value (token preferred, e.g. `var(--score-excellent)`). */
  color: string;
  value?: undefined;
  tone?: undefined;
}

type ProgressBarProps = ProgressBarToneProps | ProgressBarLegacyProps;

const TONE_FILL_CLASS: Record<ProgressBarTone, string> = {
  primary: "progress-bar__fill--primary",
  success: "progress-bar__fill--success",
  warning: "progress-bar__fill--warning",
  destructive: "progress-bar__fill--destructive",
  muted: "progress-bar__fill--muted",
};

export function ProgressBar(props: ProgressBarProps) {
  const rawValue = "value" in props && props.value !== undefined ? props.value : props.percent;
  const numeric = Number.isFinite(rawValue) ? (rawValue as number) : 0;
  const clamped = Math.max(0, Math.min(100, numeric));

  // Tone path: pure class-based color, no extra inline style beyond width.
  if ("tone" in props || props.color === undefined) {
    const tone = (props as ProgressBarToneProps).tone ?? "primary";
    return (
      <div
        className={cn("progress-bar", props.trackClassName)}
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(clamped)}
        data-tone={tone}>
        <div
          className={cn("progress-bar__fill", TONE_FILL_CLASS[tone], props.className)}
          style={{ width: `${clamped}%` }}
        />
      </div>
    );
  }

  // Bridge caller colors through a custom property instead of inline backgroundColor.
  const customStyle = {
    "--progress-color": props.color,
    width: `${clamped}%`,
  } as CSSProperties;

  return (
    <div
      className={cn("progress-bar", props.trackClassName)}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(clamped)}>
      <div
        className={cn("progress-bar__fill progress-bar__fill--custom", props.className)}
        style={customStyle}
      />
    </div>
  );
}
