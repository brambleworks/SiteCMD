import { useState, useEffect, useRef } from "react";
import type { ScanProgressEvent, MultiScanProgressEvent } from "@/hooks/useScan";
import { ProgressBar } from "@/components/ui/progress-bar";
import {
  getMultiScanOverallPercent,
  getWebScanProgressLabel,
  getWebScanProgressPercent,
} from "@/lib/scan-progress-display";
import { CATEGORY_LABELS, formatCheckName } from "@/lib/tokens";
import type { ScanRunStep } from "@/lib/scan-run-status";
import type { ScanCategory, ScheduledScanType } from "@/lib/types";
import { formatUrlDisplay } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { CheckCircle, Loader2, X, Minimize2, FileCode } from "lucide-react";
import { CODE_SCAN_STAGES, WEB_SCAN_STAGES } from "@/components/scan/scan-overlay-stages";
import { SCAN_LABELS } from "@/lib/scan-labels";

interface ScanOverlayProps {
  progress: ScanProgressEvent | null;
  multiProgress?: MultiScanProgressEvent | null;
  url: string;
  scanType?: ScheduledScanType | null;
  scanRunStep?: ScanRunStep | null;
  onCancel?: () => void;
  onMinimize?: () => void;
}

interface ScanActivityEntry {
  id: string;
  category: string;
  label: string;
  status: ScanProgressEvent["status"];
  results: number;
  checksDone: number;
  checksTotal: number;
  timestampMs: number;
  stageIndex: number;
  percent: number;
}

const ACTIVITY_LIMIT = 48;
const TERMINAL_VISIBLE_LIMIT = 12;
const SMOOTH_PROGRESS_INTERVAL_MS = 50;
const SMOOTH_PROGRESS_PERCENT_PER_SECOND = 46;
const FINAL_PROGRESS_PERCENT_PER_SECOND = 360;
const PHASE_MIN_DWELL_MS = 320;
const WEB_TAIL_PROGRESS_TICK_MS = 250;

function getWebScanStageIndex(progress: ScanProgressEvent | null): number {
  if (!progress) return 0;
  if (progress.check_id === "browser-analysis") return 7;
  if (progress.check_id === "polish-css" || progress.check_id === "polish-signals") return 6;
  if (progress.check_id === "fetch") return 0;

  switch (progress.category) {
    case "security":
      return 1;
    case "seo":
      return 2;
    case "performance":
      return 3;
    case "accessibility":
      return 4;
    case "compliance":
      return 5;
    case "config":
    case "polish":
      return 6;
    default:
      return 0;
  }
}

function getActivityStatusLabel(status: ScanProgressEvent["status"]) {
  if (status === "running") return "Running";
  if (status === "skipped") return "Skipped";
  return "Done";
}

function getTerminalStatusClass(status: ScanProgressEvent["status"]) {
  if (status === "running") return "scan-terminal-status-running";
  if (status === "skipped") return "scan-terminal-status-skipped";
  return "scan-terminal-status-complete";
}

function getCategoryLabel(category: string) {
  return CATEGORY_LABELS[category as ScanCategory] ?? formatCheckName(category);
}

function getScanActivityLabel(progress: ScanProgressEvent) {
  if (!progress.check_id.startsWith("code-scan.")) {
    return getWebScanProgressLabel(progress);
  }

  const step = progress.check_id.replace(/^code-scan\./, "");
  switch (step) {
    case "collect-files":
      return "Collect files";
    case "analyze-source":
      return "Analyze source";
    case "supply-chain":
      return "Review dependencies";
    case "operations":
      return "Review release setup";
    case "finalize":
      return "Finalize issues";
    case "save":
      return "Save results";
    case "work-items":
      return "Update issues";
    case "summary":
      return "Build summary";
    case "complete":
      return "Complete scan";
    default:
      return formatCheckName(step);
  }
}

function getCodeScanStageIndex(progress: ScanProgressEvent | null): number {
  const checkId = progress?.check_id ?? "";
  if (checkId.includes("analyze-source")) return 1;
  if (checkId.includes("supply-chain")) return 2;
  if (checkId.includes("operations")) return 3;
  if (checkId.includes("save") || checkId.includes("work-items")) return 4;
  if (checkId.includes("summary") || checkId.includes("complete") || checkId.includes("finalize")) {
    return 5;
  }
  return 0;
}

function getCodeScanProgressPercent(progress: ScanProgressEvent | null): number {
  if (!progress?.check_id.startsWith("code-scan.")) return 5;
  if (!progress.checks_total) return 5;
  return Math.min(
    100,
    Math.max(5, Math.round((progress.checks_done / progress.checks_total) * 100)),
  );
}

function getActivityStageIndex(progress: ScanProgressEvent, isCodeScan: boolean) {
  return isCodeScan ? getCodeScanStageIndex(progress) : getWebScanStageIndex(progress);
}

function getActivityPercent(progress: ScanProgressEvent, isCodeScan: boolean) {
  const rawPercent = isCodeScan
    ? getCodeScanProgressPercent(progress)
    : getWebScanProgressPercent(progress);
  return getProgressTargetPercent(rawPercent, progress, isCodeScan);
}

function getProgressTargetPercent(
  rawPercent: number,
  progress: ScanProgressEvent | null,
  isCodeScan: boolean,
  tailElapsedMs = 0,
) {
  const finalCodeProgress =
    isCodeScan && progress?.check_id === "code-scan.complete" && progress.status === "complete";
  if (finalCodeProgress) return 100;
  if (isCodeScan) return Math.min(96, Math.max(5, rawPercent));

  if (progress?.check_id === "browser-analysis") {
    if (progress.status === "complete") return 100;
    return 93 + Math.min(5, tailElapsedMs / 1_400);
  }

  if (progress?.check_id === "polish-signals") {
    if (progress.status === "complete") return 93;
    return 90 + Math.min(3, tailElapsedMs / 1_400);
  }

  return Math.min(96, Math.max(0, rawPercent));
}

function getWebTailProgressKey(progress: ScanProgressEvent | null, isCodeScan: boolean) {
  if (isCodeScan || !progress || progress.status !== "running") return null;
  if (progress.check_id === "browser-analysis" || progress.check_id === "polish-signals") {
    return progress.check_id;
  }
  return null;
}

function useSmoothedProgress(targetPercent: number, resetKey: string) {
  const [displayPercent, setDisplayPercent] = useState(targetPercent);
  // State updaters read the committed target through a ref to avoid effect-event calls.
  const targetRef = useRef(targetPercent);
  useEffect(() => {
    targetRef.current = targetPercent;
  }, [targetPercent]);

  useEffect(() => {
    setDisplayPercent(targetRef.current);
  }, [resetKey]);

  useEffect(() => {
    const interval = window.setInterval(() => {
      setDisplayPercent((current) => {
        const target = targetRef.current;
        const delta = target - current;
        if (Math.abs(delta) < 0.5) return target;

        const speed =
          target >= 100 ? FINAL_PROGRESS_PERCENT_PER_SECOND : SMOOTH_PROGRESS_PERCENT_PER_SECOND;
        const maxStep = (speed * SMOOTH_PROGRESS_INTERVAL_MS) / 1000;
        const next = current + Math.sign(delta) * Math.min(Math.abs(delta), maxStep);
        return Number(next.toFixed(1));
      });
    }, SMOOTH_PROGRESS_INTERVAL_MS);

    return () => window.clearInterval(interval);
  }, []);

  return Math.round(displayPercent);
}

function usePacedStageIndex(targetIndex: number, resetKey: string, forceTarget = false) {
  const [visibleIndex, setVisibleIndex] = useState(targetIndex);
  const targetRef = useRef(targetIndex);
  const lastChangeRef = useRef(0);

  useEffect(() => {
    lastChangeRef.current = performance.now();
  }, []);

  useEffect(() => {
    targetRef.current = targetIndex;
  }, [targetIndex]);

  useEffect(() => {
    setVisibleIndex(targetRef.current);
    lastChangeRef.current = performance.now();
  }, [resetKey]);

  useEffect(() => {
    if (!forceTarget) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- snaps the visible stage to the forced target index; scan-progress timing state
    setVisibleIndex(targetIndex);
    lastChangeRef.current = performance.now();
  }, [forceTarget, targetIndex]);

  useEffect(() => {
    if (targetIndex <= visibleIndex) {
      if (targetIndex < visibleIndex) {
        // eslint-disable-next-line react-hooks/set-state-in-effect -- rewinds the visible stage when the target index moves backward
        setVisibleIndex(targetIndex);
        lastChangeRef.current = performance.now();
      }
      return;
    }

    const elapsed = performance.now() - lastChangeRef.current;
    const delay = Math.max(PHASE_MIN_DWELL_MS - elapsed, 0);
    const timer = window.setTimeout(() => {
      setVisibleIndex((current) => {
        const next = Math.min(current + 1, targetRef.current);
        if (next !== current) lastChangeRef.current = performance.now();
        return next;
      });
    }, delay);

    return () => window.clearTimeout(timer);
  }, [targetIndex, visibleIndex]);

  return visibleIndex;
}

function usePhaseElapsedMs(phaseKey: string | null) {
  const [phaseElapsedMs, setPhaseElapsedMs] = useState(0);

  useEffect(() => {
    if (!phaseKey) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- resets the phase timer when the phase clears; paired with the interval-driven counter
      setPhaseElapsedMs(0);
      return;
    }

    const startedAt = Date.now();
    setPhaseElapsedMs(0);
    const interval = window.setInterval(() => {
      setPhaseElapsedMs(Date.now() - startedAt);
    }, WEB_TAIL_PROGRESS_TICK_MS);

    return () => window.clearInterval(interval);
  }, [phaseKey]);

  return phaseElapsedMs;
}

function formatTerminalTime(ms: number) {
  return `${Math.max(0, ms / 1000).toFixed(1)}s`;
}

export function ScanOverlay({
  progress,
  multiProgress,
  url,
  scanType,
  scanRunStep,
  onCancel,
  onMinimize,
}: ScanOverlayProps) {
  const [activityEntries, setActivityEntries] = useState<ScanActivityEntry[]>([]);
  const [startTime] = useState(() => Date.now());
  const [webStageIndex, setWebStageIndex] = useState(() => getWebScanStageIndex(progress));
  const [elapsed, setElapsed] = useState(0);
  const logRef = useRef<HTMLDivElement>(null);
  const progressIsCodeScan = progress?.check_id.startsWith("code-scan.") ?? false;
  const plannedCollectorIsCode =
    scanRunStep?.mode === "full" && scanRunStep.label === SCAN_LABELS.code;
  // Latch Code Scan once it starts so the final paint cannot fall back to Web Scan.
  const [codePhaseSeen, setCodePhaseSeen] = useState(scanType === "code" || plannedCollectorIsCode);
  if (!codePhaseSeen && (scanType === "code" || plannedCollectorIsCode || progressIsCodeScan)) {
    setCodePhaseSeen(true);
  }
  const isCodeScan = codePhaseSeen;
  const isMultiPage = !isCodeScan && Boolean(multiProgress && multiProgress.page_count > 1);
  const multiPageKey = `${multiProgress?.session_id ?? "single"}:${multiProgress?.page_index ?? 0}`;
  const activeCollector = isCodeScan ? "code" : "web";
  const scanStepKey = `${scanType ?? "web"}:${scanRunStep?.mode ?? "single"}:${activeCollector}:${multiProgress?.session_id ?? "single"}`;

  useEffect(() => {
    const interval = setInterval(() => setElapsed(Date.now() - startTime), 1000);
    return () => clearInterval(interval);
  }, [startTime]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- resets the web scan stage and activity log at the start of each scan run
    setWebStageIndex(0);
    setActivityEntries([]);
  }, [scanStepKey, startTime]);

  const prevPageRef = useRef<number>(-1);
  useEffect(() => {
    if (multiProgress && multiProgress.page_index !== prevPageRef.current) {
      prevPageRef.current = multiProgress.page_index;
      setActivityEntries([]);
    }
  }, [multiProgress]);

  useEffect(() => {
    if (!progress) return;
    const nextStageIndex = getWebScanStageIndex(progress);
    // eslint-disable-next-line react-hooks/set-state-in-effect -- advances the web scan stage monotonically from each progress event
    setWebStageIndex((current) => Math.max(current, nextStageIndex));
    const entryIsCodeScan = progress.check_id.startsWith("code-scan.");
    const entry: ScanActivityEntry = {
      id: progress.check_id,
      category: progress.category,
      label: getScanActivityLabel(progress),
      status: progress.status,
      results: progress.results_count,
      checksDone: progress.checks_done,
      checksTotal: progress.checks_total,
      timestampMs: Date.now() - startTime,
      stageIndex: getActivityStageIndex(progress, entryIsCodeScan),
      percent: getActivityPercent(progress, entryIsCodeScan),
    };
    setActivityEntries((prev) =>
      [...prev.filter((item) => item.id !== entry.id), entry].slice(-ACTIVITY_LIMIT),
    );
  }, [progress, startTime]);

  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [activityEntries]);

  const pct = getWebScanProgressPercent(progress);
  const elapsedStr = (elapsed / 1000).toFixed(1);
  const isAxePhase = progress?.check_id === "axe-core" && progress?.status === "running";

  const AXE_TIPS = [
    "Loading page in a hidden browser…",
    "Injecting axe-core accessibility engine…",
    "Testing WCAG 2.2 compliance rules…",
    "Checking color contrast ratios…",
    "Validating ARIA attributes…",
    "Inspecting heading hierarchy…",
    "Evaluating keyboard navigation…",
  ];
  const [tipIndex, setTipIndex] = useState(0);
  useEffect(() => {
    if (!isAxePhase) return;
    const interval = setInterval(() => setTipIndex((i) => (i + 1) % AXE_TIPS.length), 3000);
    return () => clearInterval(interval);
  }, [isAxePhase, AXE_TIPS.length]);

  const circ = 2 * Math.PI * 58;
  const displayUrl = formatUrlDisplay(url);
  const codeProgress = isCodeScan && progress?.check_id.startsWith("code-scan.") ? progress : null;
  const rawCodePhaseIndex = isCodeScan ? getCodeScanStageIndex(codeProgress) : 0;
  const isCodeComplete =
    codeProgress?.check_id === "code-scan.complete" && codeProgress.status === "complete";
  const displayWebStageIndex = usePacedStageIndex(webStageIndex, scanStepKey);
  const codePhaseIndex = usePacedStageIndex(rawCodePhaseIndex, scanStepKey, isCodeComplete);
  const codePhase = CODE_SCAN_STAGES[codePhaseIndex];
  const codePhaseLabel = codePhase.key === "summary" ? "Finalizing results" : codePhase.label;
  const rawCodeScanPct = isCodeScan ? getCodeScanProgressPercent(codeProgress) : 0;
  const rawDisplayPct = isCodeScan ? rawCodeScanPct : pct;
  const webTailProgressKey = getWebTailProgressKey(progress, isCodeScan);
  const webTailElapsedMs = usePhaseElapsedMs(webTailProgressKey);
  const displayPctTarget = getProgressTargetPercent(
    rawDisplayPct,
    progress,
    isCodeScan,
    webTailElapsedMs,
  );
  const lastMultiProgressEventRef = useRef(progress);
  const [multiPagePctTarget, setMultiPagePctTarget] = useState(displayPctTarget);
  // A run-key change resets multi-page progress before the first paint.
  const activeMultiPageKey = isMultiPage ? multiPageKey : null;
  const [multiPageRunKey, setMultiPageRunKey] = useState<string | null>(null);
  if (activeMultiPageKey !== multiPageRunKey) {
    setMultiPageRunKey(activeMultiPageKey);
    if (isMultiPage) setMultiPagePctTarget(0);
  }
  useEffect(() => {
    if (!isMultiPage || progress === lastMultiProgressEventRef.current) return;
    lastMultiProgressEventRef.current = progress;
    setMultiPagePctTarget(displayPctTarget);
  }, [displayPctTarget, isMultiPage, progress]);
  const overallPctTarget =
    isMultiPage && multiProgress
      ? getMultiScanOverallPercent(multiProgress, multiPagePctTarget)
      : displayPctTarget;
  const displayPct = useSmoothedProgress(overallPctTarget, scanStepKey);
  const isFullScanRun = scanRunStep?.mode === "full" && scanRunStep.stepCount > 1;
  const activeRunStep =
    isFullScanRun && isCodeScan
      ? {
          ...scanRunStep,
          stepIndex: scanRunStep.stepCount,
          label: SCAN_LABELS.code,
        }
      : scanRunStep;
  const fullScanContextLabel = isFullScanRun
    ? `${SCAN_LABELS.full} · Step ${activeRunStep?.stepIndex} of ${activeRunStep?.stepCount} · ${activeRunStep?.label}`
    : null;
  const currentWebStageIndex = isMultiPage ? getWebScanStageIndex(progress) : displayWebStageIndex;
  const webPhase = WEB_SCAN_STAGES[currentWebStageIndex] ?? WEB_SCAN_STAGES[0];
  const displayRingColor =
    isCodeScan || isMultiPage ? "var(--brand)" : (webPhase.color ?? "var(--brand)");
  const displayRingClass = isCodeScan
    ? (CODE_SCAN_STAGES[codePhaseIndex]?.pingClass ?? "scan-ring--brand")
    : isMultiPage
      ? "scan-ring--brand"
      : (webPhase.pingClass ?? "scan-ring--brand");
  const activeCatText = isCodeScan ? "text-primary" : (webPhase.textClass ?? "text-brand");
  const activeCodePhaseIcon = CODE_SCAN_STAGES[codePhaseIndex]?.icon ?? FileCode;
  const ActiveCodePhaseIcon = activeCodePhaseIcon;
  const visibleStageIndex = isCodeScan ? codePhaseIndex : currentWebStageIndex;
  const visibleActivityEntries = activityEntries
    .filter((entry) => {
      if (entry.stageIndex < visibleStageIndex) return true;
      if (entry.stageIndex > visibleStageIndex) return false;
      return entry.percent <= displayPct + 1 || (isCodeScan && isCodeComplete);
    })
    .slice(-TERMINAL_VISIBLE_LIMIT);

  return (
    <div
      className="overlay-backdrop overlay-backdrop--scan"
      role="dialog"
      aria-modal="true"
      aria-label="Scan in progress">
      <div className="scan-overlay-content" onClick={(e) => e.stopPropagation()}>
        <div className="scan-overlay-ring-wrap">
          <div className="scan-score-hero-shell">
            <div
              className={`scan-progress-ping scan-overlay-ping animate-ping ${displayRingClass}`}
            />
            <svg className="scan-overlay-ring-svg" viewBox="0 0 128 128">
              <circle
                cx="64"
                cy="64"
                r="58"
                fill="none"
                stroke="currentColor"
                strokeOpacity={0.06}
                strokeWidth="2.5"
              />
              <circle
                cx="64"
                cy="64"
                r="58"
                fill="none"
                stroke={displayRingColor}
                strokeWidth="2.5"
                strokeLinecap="round"
                strokeDasharray={`${circ}`}
                strokeDashoffset={`${circ * (1 - displayPct / 100)}`}
                className="scan-overlay-ring-progress"
              />
            </svg>
            <div className="scan-overlay-ring-label">
              <div className="scan-overlay-pct" data-testid="scan-progress-percent">
                {displayPct}
                <span className="scan-overlay-pct-unit text-muted-foreground">%</span>
              </div>
            </div>
          </div>

          <div className="scan-overlay-caption">
            {isCodeScan ? (
              <>
                <p className="eyebrow--alt scan-overlay-eyebrow text-primary">Code Scan</p>
                <p className="scan-overlay-heading">Scanning linked project code</p>
                <p className="muted-text scan-overlay-subcaption">{`Auditing linked code for ${displayUrl}`}</p>
              </>
            ) : multiProgress && multiProgress.page_count > 1 ? (
              <>
                <p className="eyebrow--alt scan-overlay-eyebrow text-primary">Web Scan</p>
                <p className="muted-text scan-overlay-page-count">
                  Page {multiProgress.page_index + 1} of {multiProgress.page_count}
                </p>
                <p className="scan-overlay-url">{formatUrlDisplay(multiProgress.current_url)}</p>
              </>
            ) : (
              <>
                <p className="eyebrow--alt scan-overlay-eyebrow text-primary">Web Scan</p>
                <p className="scan-overlay-heading">Scanning {displayUrl}</p>
              </>
            )}
            <p className="muted-text scan-overlay-elapsed">{elapsedStr}s</p>
          </div>
        </div>

        {!isCodeScan ? (
          isMultiPage ? null : (
            <div className="scan-overlay-stage-grid" data-testid="scan-stages">
              {WEB_SCAN_STAGES.map(
                ({ key, label, icon: Icon, textClass, indicatorClass }, index) => {
                  const isDone = index < displayWebStageIndex;
                  const isActive = index === displayWebStageIndex;
                  const catText = textClass ?? "text-primary";
                  const catIndicator = indicatorClass ?? "bg-primary";

                  return (
                    <div
                      key={key}
                      data-stage-state={isActive ? "active" : isDone ? "complete" : "pending"}
                      className={`scan-overlay-stage ${
                        isActive
                          ? "bg-accent scan-overlay-stage--active"
                          : isDone
                            ? ""
                            : "scan-overlay-stage--pending"
                      }`}>
                      <div className="scan-overlay-stage-icon">
                        {isDone ? (
                          <CheckCircle className={`icon-lg ${catText}`} />
                        ) : isActive ? (
                          <div className="scan-overlay-stage-icon">
                            <Icon className={`icon-lg ${catText}`} />
                            <div
                              className={`scan-overlay-stage-dot animate-pulse ${catIndicator}`}
                            />
                          </div>
                        ) : (
                          <Icon className="icon-lg text-muted-foreground" />
                        )}
                      </div>
                      <span
                        data-stage-label="true"
                        className={`scan-overlay-stage-label ${
                          isDone || isActive ? catText : "text-muted-foreground"
                        }`}>
                        {label}
                      </span>
                    </div>
                  );
                },
              )}
            </div>
          )
        ) : (
          <div
            className="scan-overlay-stage-grid scan-overlay-stage-grid--code"
            data-testid="scan-stages">
            {CODE_SCAN_STAGES.map(({ label, icon: Icon, textClass, indicatorClass }, index) => {
              const isDone = index < codePhaseIndex;
              const isActive = index === codePhaseIndex;
              const stageText = textClass ?? "text-primary";
              const stageIndicator = indicatorClass ?? "bg-primary";

              return (
                <div
                  key={label}
                  data-stage-state={isActive ? "active" : isDone ? "complete" : "pending"}
                  className={`scan-overlay-stage ${
                    isActive
                      ? "bg-accent scan-overlay-stage--active"
                      : isDone
                        ? ""
                        : "scan-overlay-stage--pending"
                  }`}>
                  <div className="scan-overlay-stage-icon">
                    {isDone ? (
                      <CheckCircle className={`icon-lg ${stageText}`} />
                    ) : isActive ? (
                      <div className="scan-overlay-stage-icon">
                        <Icon className={`icon-lg ${stageText}`} />
                        <div className={`scan-overlay-stage-dot animate-pulse ${stageIndicator}`} />
                      </div>
                    ) : (
                      <Icon className="icon-lg text-muted-foreground" />
                    )}
                  </div>
                  <span
                    data-stage-label="true"
                    className={`scan-overlay-stage-label ${
                      isDone || isActive ? stageText : "text-muted-foreground"
                    }`}>
                    {label}
                  </span>
                </div>
              );
            })}
          </div>
        )}

        <div className="scan-overlay-status">
          {isCodeScan ? (
            <>
              <div className="scan-overlay-check-row">
                <Loader2 className="icon-sm animate-spin text-primary" />
                <span className="scan-overlay-check-text">
                  <span className="text-muted-foreground">
                    {codePhase.key === "summary" ? "Finalizing " : "Checking "}
                  </span>
                  <span className="scan-overlay-check-strong">
                    {codePhase.key === "summary" ? "results" : codePhase.label}
                  </span>
                </span>
              </div>
              <div className="scan-overlay-check-detail subtitle-xs animate-pulse">
                <ActiveCodePhaseIcon className="icon-xs" />
                <span>{codePhase.detail}</span>
              </div>
            </>
          ) : progress ? (
            // Once progress exists, preserve its phase through terminal events.
            <>
              <div className="scan-overlay-check-row">
                <Loader2 className={`icon-sm animate-spin ${activeCatText}`} />
                <span className="scan-overlay-check-text">
                  <span className="text-muted-foreground">
                    {isAxePhase || webPhase.key === "browser" ? "Running " : "Checking "}
                  </span>
                  <span className="scan-overlay-check-strong">
                    {isAxePhase ? "browser metrics" : webPhase.label}
                  </span>
                </span>
              </div>
              <p className="subtitle-xs animate-pulse">
                {isAxePhase ? AXE_TIPS[tipIndex] : webPhase.detail}
              </p>
            </>
          ) : (
            <div className="scan-overlay-check-row">
              <Loader2 className="icon-sm animate-spin text-primary" />
              <span className="scan-overlay-check-text">
                <span className="text-muted-foreground">Preparing </span>
                <span className="scan-overlay-check-strong">scan</span>
              </span>
            </div>
          )}
        </div>

        <ProgressBar
          percent={displayPct}
          color={displayRingColor}
          className="scan-overlay-bar-fill"
          trackClassName="scan-overlay-bar-track"
        />

        <div className="scan-terminal" data-testid="scan-terminal">
          <div className="scan-terminal-header">
            <div className="row-tight scan-overlay-terminal-title-wrap">
              <p className="scan-terminal-title">
                {isCodeScan ? "Code Scan Events" : "Live Scan Events"}
              </p>
            </div>
            <span className="scan-terminal-meta">
              {isCodeScan
                ? codeProgress?.results_count
                  ? `${codeProgress.results_count} issues`
                  : "Local audit running"
                : progress?.checks_total
                  ? `${progress.checks_done}/${progress.checks_total}`
                  : "Waiting"}
            </span>
          </div>
          <div ref={logRef} className="scan-terminal-body">
            {visibleActivityEntries.length === 0 ? (
              <div className="scan-terminal-empty">
                <span className="scan-terminal-prompt">sitecmd</span>
                <span>
                  {activityEntries.length > 0
                    ? "syncing scan events"
                    : "waiting for the first scan event"}
                </span>
                <span className="scan-terminal-cursor" />
              </div>
            ) : null}
            {visibleActivityEntries.map((entry) => {
              const resultLabel =
                entry.status === "complete"
                  ? entry.results > 0
                    ? `${entry.results} issue${entry.results === 1 ? "" : "s"}`
                    : "No issues"
                  : entry.checksTotal > 0
                    ? `${entry.checksDone}/${entry.checksTotal}`
                    : null;
              return (
                <div key={entry.id} className="scan-terminal-row">
                  <span className="scan-terminal-time">
                    {formatTerminalTime(entry.timestampMs)}
                  </span>
                  <span className="scan-terminal-prompt">sitecmd</span>
                  <span className={getTerminalStatusClass(entry.status)}>
                    {getActivityStatusLabel(entry.status)}
                  </span>
                  <span className="scan-terminal-command">
                    <span className="scan-terminal-category">
                      {getCategoryLabel(entry.category)}
                    </span>
                    <span className="scan-terminal-separator">/</span>
                    <span>{entry.label}</span>
                  </span>
                  {resultLabel ? <span className="scan-terminal-result">{resultLabel}</span> : null}
                </div>
              );
            })}
          </div>
        </div>

        <div className="scan-overlay-footer">
          <span className="meta-num" data-testid="scan-run-context">
            {fullScanContextLabel
              ? fullScanContextLabel
              : isCodeScan
                ? `${displayPct}% complete · ${codePhaseLabel}`
                : isMultiPage && multiProgress
                  ? `${displayPct}% overall · Page ${multiProgress.page_index + 1} of ${
                      multiProgress.page_count
                    }`
                  : `${progress?.checks_done || 0} of ${progress?.checks_total || "…"} checks`}
          </span>
          <div className="scan-overlay-footer-actions">
            {onMinimize && (
              <Button
                unstyled
                type="button"
                onClick={onMinimize}
                className="icon-btn text-meta text-foreground scan-overlay-footer-btn">
                <Minimize2 className="icon-xs" /> Continue in background
              </Button>
            )}
            {onCancel && (
              <Button
                unstyled
                type="button"
                onClick={onCancel}
                className="icon-btn text-meta text-foreground scan-overlay-footer-btn">
                <X className="icon-xs" /> Cancel scan
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
