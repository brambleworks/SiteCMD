import { useState } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import { Copy, CheckCircle, ChevronRight, ExternalLink, ShieldAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ExtLink } from "@/components/ui/external-link";
import { buildCommand, getUpdateTargetVersion } from "@/components/dashboard/update-commands";
import type { PackageUpdate } from "@/lib/types";
import type { AlertItem } from "@/lib/issue-ranking";

const SEVERITY_BADGE_CLASS: Record<string, string> = {
  critical: "tone-badge--critical",
  high: "tone-badge--high",
  medium: "tone-badge--medium",
  low: "tone-badge--low",
};

export interface AlertDetailProps {
  alert: AlertItem;
  securityUpdates?: PackageUpdate[];
  nonSecurityUpdates?: PackageUpdate[];
  lastCIRun?: {
    name: string;
    conclusion: string | null;
    status: string;
    htmlUrl: string;
    updatedAt: string;
  } | null;
}

export function AlertDetail({ alert, securityUpdates, lastCIRun }: AlertDetailProps) {
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const handleCopy = async (text: string, id: string) => {
    try {
      await copyToClipboard(text);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch {
      // Clipboard access is best-effort.
    }
  };

  return (
    <div className="dossier-section-stack">
      {alert.id === "vuln-packages" && securityUpdates && securityUpdates.length > 0 && (
        <SecurityUpdatesContent
          packages={securityUpdates}
          copiedId={copiedId}
          onCopy={handleCopy}
        />
      )}
      {alert.id === "ci-failed" && lastCIRun && <CIFailureContent run={lastCIRun} />}
      {alert.id === "ssl-expiry" && <SSLExpiryDetail />}
    </div>
  );
}

function SecurityUpdatesContent({
  packages,
  copiedId,
  onCopy,
}: {
  packages: PackageUpdate[];
  copiedId: string | null;
  onCopy: (text: string, id: string) => void;
}) {
  const [expandedPkg, setExpandedPkg] = useState<string | null>(
    packages.length === 1 ? packages[0].name : null,
  );

  return (
    <div className="stack-snug">
      {packages.map((pkg) => {
        const severityClass =
          SEVERITY_BADGE_CLASS[pkg.advisorySeverity || "medium"] ?? "tone-badge--medium";
        const cmd = buildCommand(pkg);
        const targetVersion = getUpdateTargetVersion(pkg);
        const isExpanded = expandedPkg === pkg.name;

        return (
          <div key={`${pkg.ecosystem}:${pkg.name}`} className="source-group-panel">
            <Button
              unstyled
              type="button"
              onClick={() => setExpandedPkg(isExpanded ? null : pkg.name)}
              className="alert-source-row">
              <span className={`tone-badge ${severityClass}`}>
                {pkg.advisorySeverity || "vuln"}
              </span>
              <span className="flex-fill row-title">{pkg.name}</span>
              <span className="subtitle-xs no-shrink">{pkg.ecosystem}</span>
              <ChevronRight className={`disclosure-chevron ${isExpanded ? "is-open" : ""}`} />
            </Button>

            {isExpanded && (
              <div className="expand-detail stack-base">
                <div className="alert-version-row">
                  <span className="text-muted-foreground">
                    Current: <span className="text-foreground text-mono">{pkg.currentVersion}</span>
                  </span>
                  <span className="text-muted-foreground">{"\u2192"}</span>
                  <span className="text-muted-foreground">
                    Fixed release:{" "}
                    <span
                      className={targetVersion ? "text-score-excellent text-mono" : "text-mono"}>
                      {targetVersion ?? "not published"}
                    </span>
                  </span>
                </div>

                {cmd ? (
                  <div className="row-between alert-command-row">
                    <code className="alert-command-code">{cmd}</code>
                    <Button
                      variant="ghost"
                      size="sm"
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        onCopy(cmd, pkg.name);
                      }}
                      className="alert-copy-btn text-muted-foreground">
                      {copiedId === pkg.name ? (
                        <CheckCircle className="icon-sm text-score-excellent" />
                      ) : (
                        <Copy className="icon-sm" />
                      )}
                    </Button>
                  </div>
                ) : (
                  <p className="text-body-muted text-relaxed">
                    Review the advisory for mitigations or a replacement while monitoring for a
                    fixed release.
                  </p>
                )}

                {pkg.advisoryUrl && (
                  <ExtLink href={pkg.advisoryUrl} className="subtitle-xs alert-advisory-link">
                    <ExternalLink className="icon-xs" /> View advisory
                  </ExtLink>
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

function CIFailureContent({
  run,
}: {
  run: {
    name: string;
    conclusion: string | null;
    status: string;
    htmlUrl: string;
    updatedAt: string;
  };
}) {
  const timeStr = new Date(run.updatedAt).toLocaleString();

  return (
    <div className="alert-info-panel stack-card">
      <div className="stack-base">
        <div className="row-between">
          <span className="section-label-mid">Workflow</span>
          <span className="row-title-md">{run.name}</span>
        </div>
        <div className="row-between">
          <span className="section-label-mid">Status</span>
          <span className="text-body alert-status-failed">{run.conclusion || run.status}</span>
        </div>
        <div className="row-between">
          <span className="section-label-mid">Time</span>
          <span className="text-13-muted">{timeStr}</span>
        </div>
      </div>

      <ExtLink href={run.htmlUrl} className="action-link">
        <ExternalLink className="icon-md" /> View on GitHub Actions
      </ExtLink>
    </div>
  );
}

function SSLExpiryDetail() {
  return (
    <div className="alert-info-panel stack-base">
      <div className="row-start">
        <ShieldAlert className="icon-lg alert-ssl-icon" />
        <div className="stack-snug">
          <p className="text-body alert-ssl-title">SSL certificate needs renewal</p>
          <p className="text-body-muted text-relaxed">
            Your SSL/TLS certificate is expired or inside the renewal window. If it lapses, visitors
            can see browser warnings or blocked HTTPS connections.
          </p>
          <p className="text-body-muted text-relaxed">
            If you use a managed provider (Cloudflare, Let's Encrypt, AWS ACM), renewal is typically
            automatic. Check your provider's dashboard to confirm auto-renewal is enabled. For
            manually managed certificates, renew and install the new certificate before expiry.
          </p>
        </div>
      </div>
    </div>
  );
}
