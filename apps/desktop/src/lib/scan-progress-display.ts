import { formatCheckName } from "@/lib/tokens";

interface WebScanProgressLike {
  check_id: string;
  checks_done: number;
  checks_total: number;
}

export function getWebScanProgressLabel(progress: Pick<WebScanProgressLike, "check_id">): string {
  switch (progress.check_id) {
    case "fetch":
      return "Fetching page";
    case "polish-css":
      return "Fetching styles";
    case "polish-signals":
      return "Checking polish signals";
    case "browser-analysis":
      return "Running browser metrics";
    default:
      return formatCheckName(progress.check_id);
  }
}

export function getWebScanProgressDetail(progress: WebScanProgressLike | null): string {
  if (!progress) return "Starting...";
  if (progress.checks_total > 0) {
    return `${progress.checks_done} of ${progress.checks_total} checks`;
  }
  return getWebScanProgressLabel(progress);
}
