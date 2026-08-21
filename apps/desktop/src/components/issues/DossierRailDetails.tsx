import { FileText } from "lucide-react";
import { DossierRail } from "@/components/issues/IssueDossierPanel";
import { formatRelativeTime } from "@/lib/tokens";
import type { DesktopPromptEntry } from "@/lib/desktop-prompts";
import { useCurrentTime } from "@/lib/useCurrentTime";

export function DossierRecentChangeRail({
  prompt,
}: {
  prompt: Pick<DesktopPromptEntry, "title" | "detail" | "relativePath" | "updatedAt">;
}) {
  const nowMs = useCurrentTime();

  return (
    <DossierRail label="Recent file change">
      <div className="stack-snug" data-testid="recent-watched-file">
        <div className="row text-primary">
          <FileText className="icon-sm" />
          <span className="text-meta dossier-change-label">
            {formatRelativeTime(new Date(prompt.updatedAt), nowMs)}
          </span>
        </div>
        <p className="dossier-rail-mono" title={prompt.relativePath}>
          {prompt.relativePath}
        </p>
        <p className="text-meta text-relaxed">{prompt.detail}</p>
      </div>
    </DossierRail>
  );
}
