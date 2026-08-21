import { useState } from "react";
import { FileText, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { DesktopPromptEntry } from "@/lib/desktop-prompts";

interface WatchedFileArrivalBannerProps {
  prompt: DesktopPromptEntry;
  onOpenFile?: (() => void) | null;
  onReview?: (() => void) | null;
  reviewLabel?: string;
}

export function WatchedFileArrivalBanner({
  prompt,
  onOpenFile,
  onReview,
  reviewLabel = "Review matching work",
}: WatchedFileArrivalBannerProps) {
  const [dismissedId, setDismissedId] = useState<string | null>(null);
  // Reset the dismissal when a new prompt arrives, adjusting state during render
  // rather than via an effect.
  const [renderedPromptId, setRenderedPromptId] = useState(prompt.id);
  if (renderedPromptId !== prompt.id) {
    setRenderedPromptId(prompt.id);
    setDismissedId(null);
  }

  if (dismissedId === prompt.id) {
    return null;
  }

  return (
    <div className="watched-file-banner">
      <div className="recent-watched-head">
        <div className="watched-file-copy">
          <div className="row">
            <FileText className="icon-md text-primary" />
            <p className="recent-watched-title">{prompt.title}</p>
          </div>
          <p className="text-13-muted text-relaxed">{prompt.detail}</p>
          <p className="text-mono-sm text-relaxed watched-file-path">{prompt.relativePath}</p>
        </div>

        <Button
          unstyled
          type="button"
          aria-label="Dismiss watched file banner"
          className="watched-file-dismiss"
          onClick={() => setDismissedId(prompt.id)}>
          <X className="icon-md" />
        </Button>
      </div>

      {onOpenFile || onReview ? (
        <div className="watched-file-actions">
          {onOpenFile ? (
            <Button variant="ghost" className="watched-file-btn" onClick={onOpenFile}>
              Open changed file
            </Button>
          ) : null}
          {onReview ? (
            <Button variant="ghost" className="watched-file-btn" onClick={onReview}>
              {reviewLabel}
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
