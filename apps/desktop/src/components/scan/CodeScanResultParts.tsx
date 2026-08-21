import { memo } from "react";
import type { CodeIssue } from "@/lib/types";
import { IssueScopeInline } from "@/components/issues/IssueScopeSummary";
import { getGuardrailIssueScope } from "@/lib/issue-scope";
import { SEVERITY_STYLES } from "@/components/scan/code-scan-result-model";
import { ChevronRight, FileCode } from "lucide-react";
import { Button } from "@/components/ui/button";

export const CodeIssueRow = memo(function CodeIssueRow({
  issue,
  onOpen,
}: {
  issue: CodeIssue;
  onOpen: (issueId: string) => void;
}) {
  const scopeMeta = getGuardrailIssueScope(issue);
  const severityStyle = SEVERITY_STYLES[issue.severity] ?? SEVERITY_STYLES.low;

  return (
    <Button unstyled type="button" onClick={() => onOpen(issue.id)} className="code-result-row">
      <div className="code-result-evidence-card">
        <FileCode className="text-cat-code" />
      </div>
      <div className="code-result-body">
        <div className="row-wrap">
          <h3 className="code-result-title">{issue.title}</h3>
          <span className={`eyebrow ${severityStyle.labelClass}`}>{issue.severity}</span>
        </div>
        <p className="text-13-muted text-relaxed">{issue.description}</p>
        <IssueScopeInline meta={scopeMeta} />
      </div>
      <ChevronRight className="icon-md text-muted-foreground code-result-chevron" />
    </Button>
  );
});
