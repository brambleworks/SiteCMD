import { Loader2, Terminal } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DossierSection } from "@/components/issues/IssueDossierPanel";
import type { DesktopCommandResult } from "@/lib/desktop-actions";

interface CommandExecutionPanelProps {
  command: string;
  result: DesktopCommandResult | null;
  running: boolean;
  onVerify?: () => void;
  verifying?: boolean;
  verifyLabel?: string;
}

export function CommandExecutionPanel({
  command,
  result,
  running,
  onVerify,
  verifying = false,
  verifyLabel = "Verify now",
}: CommandExecutionPanelProps) {
  if (!running && !result) return null;

  return (
    <DossierSection
      label="Last command run"
      action={
        onVerify ? (
          <Button
            type="button"
            variant="ghost"
            className="command-verify-btn text-meta"
            onClick={onVerify}
            disabled={running || verifying}>
            {verifying ? <Loader2 className="spinner-sm" /> : null}
            {verifyLabel}
          </Button>
        ) : undefined
      }>
      <div className="stack-base">
        <div className="card-sunken">
          <div className="command-head-row">
            <div className="flex-fill">
              <p className="section-label">Command</p>
              <p className="mono-value-block">{command}</p>
            </div>
            <div
              className={`command-status ${
                running
                  ? "command-status--running"
                  : result?.success
                    ? "command-status--succeeded"
                    : "command-status--failed"
              }`}>
              {running ? (
                <Loader2 className="icon-xs animate-spin" />
              ) : (
                <Terminal className="icon-xs" />
              )}
              {running ? "Running" : result?.success ? "Succeeded" : "Failed"}
            </div>
          </div>
        </div>

        {result?.stdout ? <CommandOutputBlock label="stdout" value={result.stdout} /> : null}

        {result?.stderr ? (
          <CommandOutputBlock label="stderr" value={result.stderr} tone="error" />
        ) : null}
      </div>
    </DossierSection>
  );
}

function CommandOutputBlock({
  label,
  value,
  tone = "default",
}: {
  label: string;
  value: string;
  tone?: "default" | "error";
}) {
  return (
    <div className={`command-output-card ${tone === "error" ? "command-output-card--error" : ""}`}>
      <p className="section-label">{label}</p>
      <pre className="command-output-block">{value}</pre>
    </div>
  );
}
