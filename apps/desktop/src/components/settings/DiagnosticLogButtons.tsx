import { useState } from "react";
import { getLogPath, readRecentLogs } from "@/lib/commands";
import { copyToClipboard } from "@/lib/clipboard";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { buildObservabilitySnapshotText } from "@/lib/observability";
import { buildPerformanceSnapshotText } from "@/lib/performance-metrics";
import { arch, platform, version } from "@tauri-apps/plugin-os";
import { Check, Copy, FileText } from "lucide-react";
import { userFacingError } from "@/lib/user-facing-error";

export function DiagnosticLogButtons() {
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(false);
  const toast = useToast();

  const handleCopy = async () => {
    setLoading(true);
    try {
      const logs = await readRecentLogs({ lines: 500 });
      const header = `SiteCMD Diagnostic Log\nPlatform: ${platform()} ${version()} (${arch()})\n${"-".repeat(40)}\n\n`;
      const perf = buildPerformanceSnapshotText();
      const observability = buildObservabilitySnapshotText();
      await copyToClipboard(`${header}${logs}\n\n${perf}\n\n${observability}\n`);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
      toast.success(
        "Logs copied",
        "Diagnostic logs now include the latest local performance and observability snapshots.",
      );
    } catch (e) {
      toast.error("Failed to copy logs", userFacingError(e, "Nothing was written. Try again."));
    } finally {
      setLoading(false);
    }
  };

  const handleOpenFile = async () => {
    try {
      const path = await getLogPath();
      toast.info("Log file path", path);
    } catch (e) {
      toast.error(
        "Failed to get log path",
        userFacingError(e, "Check that the path still exists and SiteCMD can read it."),
      );
    }
  };

  return (
    <div className="row">
      <Button onClick={handleCopy} disabled={loading} variant="outline" size="sm">
        {copied ? (
          <>
            <Check className="icon-sm text-score-excellent" /> Copied
          </>
        ) : (
          <>
            <Copy className="icon-sm" /> Copy Logs
          </>
        )}
      </Button>
      <Button onClick={handleOpenFile} variant="outline" size="sm">
        <FileText className="icon-sm" /> Path
      </Button>
    </div>
  );
}
