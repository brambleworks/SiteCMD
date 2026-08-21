import { useState, useMemo, useCallback } from "react";
import { writeExportFile } from "@/lib/commands";
import { copyToClipboard } from "@/lib/clipboard";
import { useToast } from "@/hooks/useToast";
import { SEVERITIES, severityLabel } from "@/lib/severity";
import { Button } from "@/components/ui/button";
import { Check, Copy, Download } from "lucide-react";
import {
  generateWorkflow,
  type CodeThreshold,
  type Trigger,
  type WorkflowScanType,
} from "./cicd-workflow";

type ScanType = WorkflowScanType;

interface CICDSectionProps {
  projectPath?: string | null;
  siteUrl?: string;
}

export function CICDSection({ projectPath, siteUrl }: CICDSectionProps) {
  const { toast } = useToast();
  const [trigger, setTrigger] = useState<Trigger>("deploy");
  const [scanType, setScanType] = useState<ScanType>("health");
  const [threshold, setThreshold] = useState(80);
  const [codeThreshold, setCodeThreshold] = useState<CodeThreshold>("high");
  const [copied, setCopied] = useState(false);
  const [installed, setInstalled] = useState(false);

  const yaml = useMemo(
    () => generateWorkflow({ trigger, scanType, threshold, codeThreshold, siteUrl }),
    [trigger, scanType, threshold, codeThreshold, siteUrl],
  );

  const hasPath = !!projectPath && !projectPath.startsWith("__url__");

  const handleCopy = useCallback(async () => {
    const ok = await copyToClipboard(yaml);
    if (ok) {
      setCopied(true);
      toast("success", "Workflow YAML copied to clipboard");
      setTimeout(() => setCopied(false), 2000);
    }
  }, [yaml, toast]);

  const handleInstall = useCallback(async () => {
    if (!projectPath) return;
    try {
      const filePath = `${projectPath}/.github/workflows/sitecmd-scan.yml`;
      await writeExportFile({ path: filePath, content: yaml });
      setInstalled(true);
      toast("success", "Workflow installed - commit and push to enable");
      setTimeout(() => setInstalled(false), 4000);
    } catch (e) {
      toast("error", `Failed to install workflow: ${e}`);
    }
  }, [projectPath, yaml, toast]);

  return (
    <div className="settings-section-stack">
      <section className="card card--spacious settings-card-stack">
        <div>
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">GitHub Actions Scan Gate</h2>
          </div>
          <p className="body-muted settings-card-desc">
            Add a workflow that runs SiteCMD from GitHub Actions after a deploy, on pull requests,
            or on a daily schedule. Use it when you want a scan score to block or flag a release.
          </p>
        </div>

        <div className="settings-automation-row">
          <span className="form-label">Run scan</span>
          <div className="row-wrap">
            {[
              { id: "deploy" as Trigger, label: "After deploy" },
              { id: "pr" as Trigger, label: "Pull request" },
              { id: "schedule" as Trigger, label: "Daily" },
            ].map((t) => (
              <Button
                key={t.id}
                size="sm"
                variant={trigger === t.id ? "default" : "outline"}
                onClick={() => setTrigger(t.id)}>
                {t.label}
              </Button>
            ))}
          </div>
        </div>

        <div className="settings-automation-row">
          <span className="form-label">Check type</span>
          <div className="row-wrap">
            {[
              { id: "health" as ScanType, label: "Full website" },
              { id: "security" as ScanType, label: "Security only" },
              { id: "accessibility" as ScanType, label: "Accessibility" },
              { id: "polish" as ScanType, label: "Polish" },
              { id: "code" as ScanType, label: "Code Scan" },
            ].map((t) => (
              <Button
                key={t.id}
                size="sm"
                variant={scanType === t.id ? "default" : "outline"}
                onClick={() => setScanType(t.id)}>
                {t.label}
              </Button>
            ))}
          </div>
        </div>

        {scanType === "code" ? (
          <div className="settings-automation-row">
            <span className="form-label">Fail on</span>
            <div className="row-wrap">
              {SEVERITIES.map((severity) => (
                <Button
                  key={severity}
                  size="sm"
                  variant={codeThreshold === severity ? "default" : "outline"}
                  onClick={() => setCodeThreshold(severity)}>
                  {severityLabel(severity)}
                </Button>
              ))}
            </div>
            <span className="body-muted">
              GitHub marks the job failed when Code Scan finds this severity or higher.
            </span>
          </div>
        ) : (
          <div className="settings-automation-row">
            <span className="form-label">Fail below</span>
            <input
              type="number"
              min={0}
              max={100}
              value={threshold}
              onChange={(e) => setThreshold(Math.max(0, Math.min(100, Number(e.target.value))))}
              className="threshold-input ghost-border"
            />
            <span className="body-muted">GitHub marks the job failed when the score is lower.</span>
          </div>
        )}
      </section>

      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <div className="flex-fill">
            <h2 className="settings-card-title">Install or Copy</h2>
            <p className="body-muted settings-card-desc">
              Install writes the workflow into the linked project folder. Copy works when the folder
              is not linked or you want to paste it yourself.
            </p>
          </div>
          <Button onClick={handleInstall} disabled={!hasPath || installed}>
            <Download className="icon-xs" />
            {installed ? "Installed" : "Install Workflow"}
          </Button>
        </div>
        {!hasPath ? (
          <p className="subtitle-xs settings-card-note">
            Link a local project folder to write the workflow file automatically.
          </p>
        ) : null}
      </section>

      <section className="card card--spacious">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">.github/workflows/sitecmd-scan.yml</h2>
          <Button variant="ghost" size="sm" onClick={handleCopy}>
            {copied ? <Check className="icon-xs" /> : <Copy className="icon-xs" />}
            {copied ? "Copied" : "Copy"}
          </Button>
        </div>
        <pre className="workflow-code-block">{yaml}</pre>
      </section>
    </div>
  );
}
