import { FileText } from "lucide-react";
import { DossierSection } from "@/components/issues/IssueDossierPanel";
import { formatRelativeTime } from "@/lib/tokens";
import type { DesktopPromptEntry } from "@/lib/desktop-prompts";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface RecentWatchedFileSectionProps {
  prompt: Pick<DesktopPromptEntry, "title" | "detail" | "relativePath" | "updatedAt">;
}

export function RecentWatchedFileSection({ prompt }: RecentWatchedFileSectionProps) {
  const nowMs = useCurrentTime();

  return (
    <DossierSection label="Recent watched file">
      <div className="recent-watched-card">
        <div className="recent-watched-head">
          <div className="flex-fill">
            <div className="row">
              <FileText className="icon-md text-primary" />
              <p className="recent-watched-title">{prompt.title}</p>
            </div>
            <p className="text-mono-sm text-relaxed recent-watched-path">{prompt.relativePath}</p>
          </div>
          <span className="text-micro recent-watched-time">
            {formatRelativeTime(new Date(prompt.updatedAt), nowMs)}
          </span>
        </div>

        <p className="text-13-muted text-relaxed recent-watched-detail">{prompt.detail}</p>
      </div>
    </DossierSection>
  );
}
