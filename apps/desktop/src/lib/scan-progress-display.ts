import { formatCheckName } from "@/lib/tokens";

interface WebScanProgressLike {
  check_id: string;
  status: string;
  checks_done: number;
  checks_total: number;
}

interface MultiScanProgressLike {
  page_index: number;
  page_count: number;
  page_status: string;
}

export function getMultiScanOverallPercent(
  progress: MultiScanProgressLike,
  currentPagePercent: number,
): number {
  if (progress.page_count <= 1) return Math.min(100, Math.max(0, currentPagePercent));

  const terminalPage = progress.page_status === "complete" || progress.page_status === "error";
  const pageFraction = terminalPage ? 1 : Math.min(1, Math.max(0, currentPagePercent / 100));
  return Math.min(
    100,
    Math.max(0, ((progress.page_index + pageFraction) / progress.page_count) * 100),
  );
}

export function getWebScanProgressPercent(progress: WebScanProgressLike | null): number {
  if (!progress) return 0;

  if (progress.checks_total > 0) {
    const ratio = progress.checks_done / Math.max(progress.checks_total, 1);
    return Math.min(70, Math.max(8, Math.round(8 + ratio * 62)));
  }

  switch (progress.check_id) {
    case "fetch":
      return progress.status === "complete" ? 8 : 4;
    case "polish-css":
      return progress.status === "complete" ? 72 : 70;
    case "polish-signals":
      return progress.status === "complete" ? 75 : 72;
    case "browser-analysis":
      return progress.status === "complete" ? 99 : 75;
    default:
      return progress.status === "complete" ? 75 : 70;
  }
}

export function getWebScanProgressLabel(progress: WebScanProgressLike): string {
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

export function getWebScanProgressInline(progress: WebScanProgressLike | null): string {
  if (!progress) return "Starting scan...";
  const pct = getWebScanProgressPercent(progress);
  if (progress.checks_total > 0) {
    return `${progress.checks_done}/${progress.checks_total} checks • ${pct}%`;
  }
  return `${getWebScanProgressLabel(progress)} • ${pct}%`;
}
