import { useCallback, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { buildScanSummaryModel, type ScanSummaryModel } from "@/components/scan/scan-summary-model";
import type { ScanSessionSummary, ScanSummary } from "@/hooks/useHistory";
import type { MultiScanResult, ScanState } from "@/hooks/useScan";
import { normalizeAppUrlForKey } from "@/lib/app-targets";
import { loadCurrentScoreSnapshot } from "@/lib/current-score";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import { buildProjectIssueSummaryFromSnapshot } from "@/lib/project-nav-badges";
import { getProjectNavBadgeSnapshot } from "@/lib/project-summary-signals";
import type { CodeScanResult, CodeScanSummary, ScanResult, ScoreSnapshot } from "@/lib/types";
import { fetchInactiveKeys } from "@/pages/issues/useInactiveIssueKeys";

interface UsePostScanSummaryParams {
  state: ScanState;
  result: ScanResult | null;
  codeResult: CodeScanResult | null;
  multiResult: MultiScanResult | null;
  activeProjectId: number | null;
  activeEnvUrl: string | null;
  activeScanScope: string;
  fullScanStillRunning: boolean;
  /** Reactive background state used by the memoized summary gate. */
  scanBackgrounded: boolean;
  /** Prevents background code reports from opening a post-scan overlay. */
  codeResultFromBackground: boolean;
  history: ScanSummary[];
  codeHistory: CodeScanSummary[];
  sessions: ScanSessionSummary[];
  showScanConfig: boolean;
}

interface UsePostScanSummaryReturn {
  scanSummary: ScanSummaryModel | null;
  showScanSummary: boolean;
  closeScanSummary: () => void;
}

export function usePostScanSummary({
  state,
  result,
  codeResult,
  multiResult,
  activeProjectId,
  activeEnvUrl,
  activeScanScope,
  fullScanStillRunning,
  scanBackgrounded,
  codeResultFromBackground,
  history,
  codeHistory,
  sessions,
  showScanConfig,
}: UsePostScanSummaryParams): UsePostScanSummaryReturn {
  const queryClient = useQueryClient();
  const [dismissedScanSummaryKey, setDismissedScanSummaryKey] = useState<string | null>(null);
  const [postScanScore, setPostScanScore] = useState<{
    key: string;
    score: ScoreSnapshot | null;
    failed: boolean;
    inactiveCheckIds: string[];
    persistedSummary: ProjectIssueSummary | null;
  } | null>(null);

  const scanSummaryScoreKey = [
    activeProjectId ?? "none",
    activeEnvUrl ?? "none",
    result?.timestamp ?? "no-web",
    codeResult?.id ?? "no-code",
    multiResult?.sessionId ?? "no-multi",
  ].join(":");

  useEffect(() => {
    if (
      state !== "complete" ||
      fullScanStillRunning ||
      scanBackgrounded ||
      activeProjectId == null ||
      activeEnvUrl == null ||
      (!result && !codeResult && !multiResult)
    ) {
      return;
    }

    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- initialize the async score load
    setPostScanScore({
      key: scanSummaryScoreKey,
      score: null,
      failed: false,
      inactiveCheckIds: [],
      persistedSummary: null,
    });
    Promise.all([
      loadCurrentScoreSnapshot(activeProjectId, activeEnvUrl),
      // Use the coalesced active-issue projection so summary counts match Issues.
      fetchInactiveKeys(queryClient, activeProjectId, normalizeAppUrlForKey(activeEnvUrl)),
      // Reuse the navigation badge snapshot for canonical issue counts.
      getProjectNavBadgeSnapshot(queryClient, activeProjectId, activeEnvUrl, {
        forceRefresh: true,
      }),
    ])
      .then(([score, inactiveKeys, snapshot]) => {
        if (cancelled) return;
        const inactiveCheckIds = Array.from(inactiveKeys);
        const persistedSummary = snapshot ? buildProjectIssueSummaryFromSnapshot(snapshot) : null;
        setPostScanScore({
          key: scanSummaryScoreKey,
          score,
          failed: false,
          inactiveCheckIds,
          persistedSummary,
        });
      })
      .catch(() => {
        if (!cancelled) {
          setPostScanScore({
            key: scanSummaryScoreKey,
            score: null,
            failed: true,
            inactiveCheckIds: [],
            persistedSummary: null,
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    activeEnvUrl,
    activeProjectId,
    codeResult,
    fullScanStillRunning,
    multiResult,
    queryClient,
    result,
    scanBackgrounded,
    scanSummaryScoreKey,
    state,
  ]);

  const scanSummary = useMemo(() => {
    if (state !== "complete" || fullScanStillRunning || scanBackgrounded) {
      return null;
    }
    const needsPersistedScore =
      activeProjectId != null && activeEnvUrl != null && (result || codeResult || multiResult);
    const persistedScore = postScanScore?.key === scanSummaryScoreKey ? postScanScore.score : null;
    const persistedScoreFailed =
      postScanScore?.key === scanSummaryScoreKey ? postScanScore.failed : false;
    const inactiveCheckIds =
      postScanScore?.key === scanSummaryScoreKey ? postScanScore.inactiveCheckIds : [];
    const persistedSummary =
      postScanScore?.key === scanSummaryScoreKey ? postScanScore.persistedSummary : null;
    if (needsPersistedScore && !persistedScore && !persistedScoreFailed) return null;
    // Per-page occurrences cannot produce an honest deduplicated headline.
    if (needsPersistedScore && multiResult && !persistedSummary) return null;
    return buildScanSummaryModel({
      result,
      codeResult,
      multiResult,
      sitecmdScore: persistedScore?.overall ?? null,
      history,
      codeHistory,
      sessions,
      scopeLabel: activeScanScope,
      inactiveCheckIds: new Set(inactiveCheckIds),
      persistedSummary,
    });
  }, [
    activeScanScope,
    codeHistory,
    codeResult,
    activeEnvUrl,
    activeProjectId,
    fullScanStillRunning,
    history,
    multiResult,
    postScanScore,
    result,
    scanBackgrounded,
    scanSummaryScoreKey,
    sessions,
    state,
  ]);

  // Background refreshes update the model without opening the announcement.
  const showScanSummary =
    scanSummary != null &&
    dismissedScanSummaryKey !== scanSummary.id &&
    !showScanConfig &&
    !codeResultFromBackground;

  const closeScanSummary = useCallback(() => {
    if (scanSummary) {
      setDismissedScanSummaryKey(scanSummary.id);
    }
  }, [scanSummary]);

  return { scanSummary, showScanSummary, closeScanSummary };
}
