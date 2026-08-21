import { useEffect, useRef } from "react";

import type { AppTarget } from "@/lib/app-targets";

import { WorkflowFollowUpBanner } from "./ScanFollowUpBanner";

interface ShellFollowUpBannerModel {
  id: string;
  title: string;
  description: string;
  actionLabel: string;
  tone: "followup" | "urgent";
  target: AppTarget;
}

interface ScanFollowUpBannerHostProps {
  page: string;
  scanState: string;
  banner: ShellFollowUpBannerModel | null;
  onOpenTarget: (target: AppTarget) => void;
  onClearBanner: () => void;
}

export function ScanFollowUpBannerHost({
  page,
  scanState,
  banner,
  onOpenTarget,
  onClearBanner,
}: ScanFollowUpBannerHostProps) {
  const visibleBannerIdRef = useRef<string | null>(null);
  const isVisible = banner != null && page === banner.target.page;

  useEffect(() => {
    if (scanState !== "scanning") return;
    visibleBannerIdRef.current = null;
    if (banner) {
      onClearBanner();
    }
  }, [banner, onClearBanner, scanState]);

  useEffect(() => {
    if (isVisible && banner) {
      visibleBannerIdRef.current = banner.id;
      return;
    }

    if (banner && visibleBannerIdRef.current === banner.id) {
      visibleBannerIdRef.current = null;
      onClearBanner();
    }
  }, [banner, isVisible, onClearBanner]);

  if (!isVisible || !banner) return null;

  return (
    <WorkflowFollowUpBanner
      className="followup-banner-host"
      title={banner.title}
      description={banner.description}
      actionLabel={banner.actionLabel}
      tone={banner.tone}
      onAction={() => onOpenTarget(banner.target)}
      onDismiss={() => {
        visibleBannerIdRef.current = null;
        onClearBanner();
      }}
    />
  );
}
