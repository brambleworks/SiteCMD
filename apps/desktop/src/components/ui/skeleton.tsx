import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

type SkeletonVariant =
  | "line"
  | "line-lg"
  | "title"
  | "heading"
  | "stat"
  | "avatar"
  | "avatar-lg"
  | "badge"
  | "pill"
  | "button"
  | "block"
  | "dot";

type SkeletonWidth = "full" | "wide" | "half" | "narrow" | "xs" | "sm" | "md" | "lg";

interface SkeletonProps {
  variant?: SkeletonVariant;
  width?: SkeletonWidth;
  className?: string;
}

export function Skeleton({ variant = "line", width, className }: SkeletonProps) {
  return (
    <div
      aria-hidden="true"
      className={cn("skeleton", `skeleton--${variant}`, width && `skeleton-w--${width}`, className)}
    />
  );
}

/** Inline skeleton - sits inside text flows without breaking layout */
export function InlineSkeleton({ variant = "title", width = "xs", className }: SkeletonProps) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "skeleton skeleton--inline",
        `skeleton--${variant}`,
        width && `skeleton-w--${width}`,
        className,
      )}
    />
  );
}

interface LoadingRegionProps {
  label: string;
  className?: string;
  children: ReactNode;
}

/** Shared accessible boundary for page- and section-shaped loading skeletons. */
export function LoadingRegion({ label, className, children }: LoadingRegionProps) {
  return (
    <div
      role="status"
      aria-label={label}
      aria-live="polite"
      aria-busy="true"
      className={cn(className)}>
      <span className="sr-only">{label}</span>
      {children}
    </div>
  );
}
