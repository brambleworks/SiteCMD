import { useState } from "react";
import { CheckCircle, ChevronRight, Copy, X } from "lucide-react";
import { copyToClipboard } from "@/lib/clipboard";
import type { CheckResult } from "@/lib/types";
import { CATEGORY_LABELS } from "@/lib/tokens";
import { Button } from "@/components/ui/button";
import { Markdown } from "@/components/ui/markdown";
import { getSeverityConfig } from "./dashboard-issue-severity";

export function IssueListOverlay({
  title,
  issues,
  onClose,
  onOpenIssue,
}: {
  title: string;
  issues: CheckResult[];
  onClose: () => void;
  onOpenIssue: (issue: CheckResult) => void;
}) {
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(
    issues.length === 1 ? issues[0].checkId : null,
  );

  const handleCopy = async (text: string, id: string) => {
    try {
      await copyToClipboard(text);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 2000);
    } catch {
      // clipboard write failed
    }
  };

  const handleCopyAll = async () => {
    const all = issues
      .map((issue) => {
        const fix = issue.fixPrompt || issue.manualFix || "";
        return `## ${issue.title} (${issue.severity})\n${issue.description}\n${fix ? `\nFix:\n${fix}` : ""}`;
      })
      .join("\n\n---\n\n");

    try {
      await copyToClipboard(all);
      setCopiedId("__all__");
      setTimeout(() => setCopiedId(null), 2000);
    } catch {
      // clipboard write failed
    }
  };

  return (
    <div className="issue-overlay-backdrop" onClick={onClose}>
      <div className="issue-overlay-scrim" />
      <div className="issue-overlay-panel" onClick={(event) => event.stopPropagation()}>
        <div className="row-between">
          <h2 className="text-lg-bold">{title}</h2>
          <div className="row">
            {issues.length > 0 ? (
              <Button
                variant="outline"
                size="sm"
                onClick={handleCopyAll}
                className="overlay-header-btn text-meta">
                {copiedId === "__all__" ? (
                  <>
                    <CheckCircle className="icon-sm text-score-excellent" /> Copied all
                  </>
                ) : (
                  <>
                    <Copy className="icon-sm" /> Copy all fix prompts
                  </>
                )}
              </Button>
            ) : null}
            <Button
              variant="ghost"
              size="icon"
              onClick={onClose}
              aria-label="Close"
              className="overlay-close-btn text-muted-foreground">
              <X className="icon-md" />
            </Button>
          </div>
        </div>

        <div className="stack-snug">
          {issues.map((issue) => {
            const sev = getSeverityConfig(issue.severity);
            const fixText = issue.fixPrompt || issue.manualFix || "";
            const isExpanded = expandedId === issue.checkId;
            return (
              <div key={issue.checkId} className="source-group-panel">
                <Button
                  unstyled
                  onClick={() => setExpandedId(isExpanded ? null : issue.checkId)}
                  className="issue-source-list-row">
                  <span className={`issue-overlay-sev ${sev.color}`}>{issue.severity}</span>
                  <span className="flex-fill row-title">{issue.title}</span>
                  <span className="subtitle-xs issue-overlay-cat">
                    {CATEGORY_LABELS[issue.category] || issue.category}
                  </span>
                  <ChevronRight className={`disclosure-chevron ${isExpanded ? "is-open" : ""}`} />
                </Button>

                {isExpanded ? (
                  <div className="issue-overlay-detail stack-base">
                    {issue.description ? (
                      <p className="body-text-muted">{issue.description}</p>
                    ) : null}

                    {fixText ? (
                      <div>
                        <div className="row-between issue-overlay-fix-head">
                          <p className="section-label-mid">How to Fix</p>
                          <Button
                            unstyled
                            onClick={(event) => {
                              event.stopPropagation();
                              handleCopy(fixText, issue.checkId);
                            }}
                            className="muted-text-action">
                            {copiedId === issue.checkId ? (
                              <>
                                <CheckCircle className="icon-xs text-score-excellent" /> Copied
                              </>
                            ) : (
                              <>
                                <Copy className="icon-xs" /> Copy
                              </>
                            )}
                          </Button>
                        </div>
                        <div className="issue-overlay-fix-body">
                          <Markdown>{fixText}</Markdown>
                        </div>
                      </div>
                    ) : (
                      <p className="body-muted text-italic">
                        No automated fix available for this issue.
                      </p>
                    )}

                    <div className="row-end">
                      <Button
                        unstyled
                        onClick={(event) => {
                          event.stopPropagation();
                          onClose();
                          onOpenIssue(issue);
                        }}
                        className="overlay-dossier-link">
                        Open dossier
                      </Button>
                    </div>
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
