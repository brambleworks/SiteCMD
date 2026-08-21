import { useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { Markdown } from "@/components/ui/markdown";
import { Button } from "@/components/ui/button";
import { copyToClipboard } from "@/lib/clipboard";
import {
  DossierAttentionSection,
  DossierNextStepsSection,
} from "@/components/issues/DossierStandardSections";
import type { PackageUpdate } from "@/lib/types";

export function UpdateSecurityAdvisorySection({ update }: { update: PackageUpdate }) {
  const hasFixedRelease = Boolean(update.advisoryFixedVersion);
  return (
    <DossierAttentionSection>
      <p className="text-body-muted text-relaxed text-foreground">
        {hasFixedRelease
          ? "A verified fixed release is available"
          : "No fixed release is published"}
        {update.advisorySeverity ? ` (${update.advisorySeverity} severity)` : ""}.
        {update.advisoryUrl ? (
          <>
            {" "}
            Advisory: <span className="font-mono">{update.advisoryUrl}</span>
          </>
        ) : null}
      </p>
    </DossierAttentionSection>
  );
}

export function UpdateNoFixSection() {
  return (
    <DossierNextStepsSection>
      <p className="text-body-muted text-relaxed">
        Review the advisory for reachable code paths. Disable or isolate the affected feature,
        replace the package when practical, and monitor the advisory for a verified fixed release.
      </p>
    </DossierNextStepsSection>
  );
}

export function UpdateBestFirstFixSection({
  children,
  command,
  onCopy,
}: {
  children?: ReactNode;
  command: string;
  onCopy?: () => void;
}) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    const ok = await copyToClipboard(command);
    if (!ok) return;
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
    onCopy?.();
  };

  return (
    <DossierNextStepsSection>
      <div className="card-sunken update-command-block group">
        <Markdown>{["```bash", command, "```"].join("\n")}</Markdown>
        <Button
          unstyled
          type="button"
          onClick={handleCopy}
          aria-label={copied ? "Command copied" : "Copy command"}
          className="code-copy-button code-copy-button--persistent">
          {copied ? (
            <Check className="icon-xs text-score-excellent" />
          ) : (
            <Copy className="icon-xs" />
          )}
        </Button>
      </div>
      {children}
    </DossierNextStepsSection>
  );
}
