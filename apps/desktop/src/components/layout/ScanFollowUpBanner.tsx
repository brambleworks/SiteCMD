import { AlertTriangle, ArrowRight, Sparkles, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface ScanFollowUpBannerProps {
  title: string;
  description: string;
  actionLabel: string;
  onAction: () => void;
  onDismiss: () => void;
  className?: string;
  tone?: "followup" | "urgent";
}

export function WorkflowFollowUpBanner({
  title,
  description,
  actionLabel,
  onAction,
  onDismiss,
  className,
  tone = "followup",
}: ScanFollowUpBannerProps) {
  const isUrgent = tone === "urgent";

  return (
    <div className={cn("card card--spacious", isUrgent ? "panel--warning" : "", className)}>
      <div className="followup-banner-row">
        <div
          className={cn(
            "followup-banner-icon ghost-border",
            isUrgent ? "followup-banner-icon--urgent text-amber-300" : "bg-muted text-primary",
          )}>
          {isUrgent ? <AlertTriangle className="icon-md" /> : <Sparkles className="icon-md" />}
        </div>
        <div className="flex-fill">
          <p className="followup-banner-title">{title}</p>
          <p className="text-13-muted text-relaxed followup-banner-desc">{description}</p>
          <div className="followup-banner-actions">
            <Button variant={isUrgent ? "accent" : "outline"} size="sm" onClick={onAction}>
              {actionLabel}
              <ArrowRight className="icon-sm" />
            </Button>
            <Button variant="ghost" size="sm" onClick={onDismiss}>
              Dismiss
            </Button>
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          type="button"
          onClick={onDismiss}
          className="followup-banner-dismiss"
          title="Dismiss follow-up">
          <X className="icon-md" />
        </Button>
      </div>
    </div>
  );
}
