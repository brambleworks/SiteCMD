import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { runScanExecution } from "@/lib/commands";
import { getCodeScanDetail } from "@/lib/scan-execution-adapters";
import { createScanActionKey } from "@/lib/scan-action-key";
import { Button } from "@/components/ui/button";
import type { CodeIssue, CodeScanResult, CodeScanSummary } from "@/lib/types";
import { normalizeCodeScanResult } from "@/lib/code-scan-result-normalize";
import { buildPendingVerificationId, resolvePendingVerification } from "@/lib/pending-verification";
import { useToast } from "@/hooks/useToast";
import { userFacingError } from "@/lib/user-facing-error";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { normalizeAppUrlForKey, type AppTarget } from "@/lib/app-targets";
import type { NavTarget } from "@/components/layout/nav-page";
import { primeLatestCodeScanSnapshot } from "@/lib/project-summary-signals";
import { fixHandoffKey, getFixHandoff, subscribeFixHandoff } from "@/lib/fix-handoff-store";
import {
  computeCodeScanComparison,
  getPreviousCodeScanSummary,
  sortCodeIssues,
} from "@/lib/code-scan-comparison";
import { CodeIssueDossier } from "@/components/scan/CodeIssueDossier";
import { CodeIssueRow } from "@/components/scan/CodeScanResultParts";
import {
  CATEGORY_LABELS,
  CATEGORY_ORDER,
  getCodeScanPresentation,
} from "@/components/scan/code-scan-result-model";
import {
  CodeScanComparisonSection,
  CodeScanEmptyState,
  CodeScanHeaderSection,
} from "@/components/scan/CodeScanResultsSections";

export { CodeIssueDossier } from "@/components/scan/CodeIssueDossier";

// Matches ISSUE_PAGE_SIZE in IssueList so both surfaces page identically.
const CODE_ISSUE_PAGE_SIZE = 100;

interface PagedCategoryGroup {
  category: string;
  issues: CodeIssue[];
  totalCount: number;
}

interface CodeScanResultsProps {
  result: CodeScanResult;
  projectPath?: string | null;
  codeHistory?: CodeScanSummary[];
  initialIssueId?: string | null;
  onOpenScanConfig?: () => void;
  onResultUpdated?: (result: CodeScanResult) => void;
  onOpenTarget?: (target: AppTarget) => void;
  onNavigate?: (page: NavTarget) => void;
}

export function CodeScanResults({
  result,
  projectPath,
  codeHistory = [],
  initialIssueId = null,
  onOpenScanConfig,
  onResultUpdated,
  onOpenTarget,
  onNavigate,
}: CodeScanResultsProps) {
  const presentation = useMemo(() => getCodeScanPresentation(), []);
  const toast = useToast();
  const [currentResult, setCurrentResult] = useState(result);
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(initialIssueId);
  const [issuePage, setIssuePage] = useState(1);
  const [previousResult, setPreviousResult] = useState<CodeScanResult | null>(null);
  const [comparisonLoading, setComparisonLoading] = useState(false);
  const [comparisonError, setComparisonError] = useState(false);
  const [comparisonRevision, setComparisonRevision] = useState(0);
  const [verifyingIssueId, setVerifyingIssueId] = useState<string | null>(null);
  // Retain the selected issue through verification so its progress modal can finish.
  const [retainedIssue, setRetainedIssue] = useState<CodeIssue | null>(null);
  const foundIssue = useMemo(
    () => currentResult.issues.find((issue) => issue.id === selectedIssueId) ?? null,
    [currentResult.issues, selectedIssueId],
  );
  const selectedIssue =
    foundIssue ?? (retainedIssue?.id === selectedIssueId ? retainedIssue : null);
  // Share the handoff store so modal and dossier state cannot diverge.
  const selectionHandoffKey = selectedIssue
    ? fixHandoffKey(
        currentResult.projectId,
        normalizeAppUrlForKey(currentResult.environmentUrl ?? ""),
        selectedIssue.checkId,
      )
    : null;
  const selectionHandoff = useSyncExternalStore(subscribeFixHandoff, () =>
    selectionHandoffKey === null ? null : getFixHandoff(selectionHandoffKey),
  );
  const previousSummary = useMemo(
    () => getPreviousCodeScanSummary(currentResult, codeHistory),
    [codeHistory, currentResult],
  );
  const comparison = useMemo(
    () => (previousResult ? computeCodeScanComparison(previousResult, currentResult) : null),
    [previousResult, currentResult],
  );
  const topIssue = useMemo(
    () => [...currentResult.issues].sort(sortCodeIssues)[0] ?? null,
    [currentResult.issues],
  );

  // Synchronize prop-derived state during render to avoid a stale frame.
  useResetOnChange(result, () => setCurrentResult(result));
  useResetOnChange(initialIssueId, () => setSelectedIssueId(initialIssueId));

  // Drop removed selections after render-time prop synchronization.
  useEffect(() => {
    if (foundIssue) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- keeps the retained copy in step with the live lookup
      setRetainedIssue(foundIssue);
      return;
    }
    if (!selectedIssueId) return;
    // Only an active handoff can keep a disappeared issue selected.
    if (selectionHandoff !== null) return;
    setSelectedIssueId(null);
    setRetainedIssue(null);
  }, [foundIssue, selectedIssueId, selectionHandoff]);

  useEffect(() => {
    let cancelled = false;
    if (!previousSummary) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- clear absent comparison state
      setPreviousResult(null);
      setComparisonLoading(false);
      setComparisonError(false);
      return;
    }

    setComparisonLoading(true);
    setComparisonError(false);
    getCodeScanDetail({ scanId: previousSummary.id })
      .then((detail) => {
        if (!cancelled) {
          setPreviousResult(detail);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setPreviousResult(null);
          setComparisonError(true);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setComparisonLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [comparisonRevision, previousSummary]);

  // Group once by category to avoid duplicate domain/category headings.
  const categoryGroups = useMemo(() => {
    const groups = new Map<string, CodeIssue[]>();
    for (const issue of currentResult.issues) {
      if (!groups.has(issue.category)) groups.set(issue.category, []);
      groups.get(issue.category)!.push(issue);
    }

    const order = [
      ...CATEGORY_ORDER.filter((category) => groups.has(category)),
      ...Array.from(groups.keys()).filter((category) => !CATEGORY_ORDER.includes(category)),
    ];

    return order.map((category) => ({
      category,
      issues: [...(groups.get(category) ?? [])].sort(sortCodeIssues),
    }));
  }, [currentResult]);

  useResetOnChange(currentResult.id, () => setIssuePage(1));

  const totalVisibleIssues = useMemo(
    () => categoryGroups.reduce((sum, group) => sum + group.issues.length, 0),
    [categoryGroups],
  );
  const totalIssuePages = Math.max(1, Math.ceil(totalVisibleIssues / CODE_ISSUE_PAGE_SIZE));
  const currentIssuePage = Math.min(issuePage, totalIssuePages);

  // Window the flattened rows to CODE_ISSUE_PAGE_SIZE across groups; headers keep full counts.
  const pagedCategoryGroups = useMemo(() => {
    const pageStart = (currentIssuePage - 1) * CODE_ISSUE_PAGE_SIZE;
    const pageEnd = pageStart + CODE_ISSUE_PAGE_SIZE;
    let offset = 0;
    const paged: PagedCategoryGroup[] = [];
    for (const group of categoryGroups) {
      const start = Math.max(pageStart - offset, 0);
      const end = Math.min(pageEnd - offset, group.issues.length);
      offset += group.issues.length;
      if (start >= end) continue;
      paged.push({
        category: group.category,
        issues: group.issues.slice(start, end),
        totalCount: group.issues.length,
      });
    }
    return paged;
  }, [currentIssuePage, categoryGroups]);

  const handleOpenIssue = useCallback((issueId: string) => {
    setSelectedIssueId(issueId);
  }, []);

  const handleVerifyIssue = useCallback(
    async (issue: CodeIssue) => {
      if (!projectPath || verifyingIssueId) return;
      const normalizedUrl = normalizeAppUrlForKey(currentResult.environmentUrl);
      setVerifyingIssueId(issue.id);
      try {
        const execution = await runScanExecution({
          request: {
            projectId: currentResult.projectId,
            environmentId: null,
            environmentUrl: currentResult.environmentUrl,
            requestedMode: "code",
            webFocus: null,
            urls: [],
            enabledCategories: null,
            timeoutSecs: null,
            axeEnabled: null,
            projectPath,
            inspectLocalDatabases: false,
            scanRequestId: null,
            retention: null,
            trigger: "verification",
            idempotencyKey: createScanActionKey("verification-code"),
          },
        });
        if (!execution.codeResult) {
          throw new Error(execution.execution.codeDetail ?? "Code verification produced no result");
        }
        const rawNextResult = execution.codeResult;
        const summaryResult = normalizeCodeScanResult(rawNextResult);
        const detailResult = await getCodeScanDetail({
          scanId: summaryResult.id,
        }).catch(() => null);
        if (!detailResult && summaryResult.issueCount > 0) {
          throw new Error("Code scan finished, but issue details could not load. Try again.");
        }
        const nextResult = detailResult ? normalizeCodeScanResult(detailResult) : summaryResult;

        primeLatestCodeScanSnapshot(nextResult);
        setCurrentResult(nextResult);
        onResultUpdated?.(nextResult);

        const stillPresent =
          nextResult.issues.find((candidate) => candidate.id === issue.id) ?? null;
        const diffSummary = [
          stillPresent ? "Still detected" : "No longer detected",
          `Critical ${currentResult.criticalCount} -> ${nextResult.criticalCount}`,
          `High ${currentResult.highCount} -> ${nextResult.highCount}`,
          `Issues ${currentResult.issueCount} -> ${nextResult.issueCount}`,
        ].join(" | ");

        if (stillPresent) {
          setSelectedIssueId(stillPresent.id);
          toast.warning("Code issue still needs attention", diffSummary);
        } else {
          if (normalizedUrl) {
            resolvePendingVerification(
              buildPendingVerificationId(
                currentResult.projectId,
                normalizedUrl,
                issue.id,
                "issues",
              ),
            );
          }
          setSelectedIssueId(null);
          toast.success("Code issue cleared", diffSummary);
        }
      } catch (error) {
        toast.error(
          "Verification failed",
          userFacingError(error, "Run the verification again after the site has deployed."),
        );
      } finally {
        setVerifyingIssueId((current) => (current === issue.id ? null : current));
      }
    },
    [currentResult, onResultUpdated, projectPath, toast, verifyingIssueId],
  );

  return (
    <div className="page-content stack-hero">
      <CodeScanHeaderSection
        currentResult={currentResult}
        presentation={presentation}
        projectPath={projectPath}
        topIssue={topIssue}
        onOpenScanConfig={onOpenScanConfig}
        onFocusTopIssue={() => {
          if (!topIssue) return;
          setSelectedIssueId(topIssue.id);
          document.getElementById("code-scan-findings")?.scrollIntoView({
            behavior: "smooth",
            block: "start",
          });
        }}
      />

      <CodeScanComparisonSection
        comparison={comparison}
        comparisonError={comparisonError}
        comparisonLoading={comparisonLoading}
        currentResult={currentResult}
        onRetryComparison={() => setComparisonRevision((value) => value + 1)}
      />

      {currentResult.issueCount === 0 ? (
        <CodeScanEmptyState presentation={presentation} />
      ) : (
        <>
          <div className="code-scan-groups">
            {pagedCategoryGroups.map((group, index) => (
              <div
                key={group.category}
                id={index === 0 ? "code-scan-findings" : undefined}
                className="stack-card">
                <div className="code-scan-group-head">
                  <h2 className="text-meta code-scan-group-label">
                    {CATEGORY_LABELS[group.category] ?? group.category} - {group.totalCount} Issue
                    {group.totalCount !== 1 ? "s" : ""}
                  </h2>
                  <div className="code-scan-group-rule" />
                </div>
                <div className="stack-base">
                  {group.issues.map((issue) => (
                    <CodeIssueRow key={issue.id} issue={issue} onOpen={handleOpenIssue} />
                  ))}
                </div>
              </div>
            ))}
          </div>

          {totalIssuePages > 1 ? (
            <div className="panel panel--flush panel--muted code-scan-pager">
              <Button
                variant="outline"
                size="sm"
                aria-label="Previous issues page"
                onClick={() => setIssuePage((page) => Math.max(1, page - 1))}
                disabled={currentIssuePage === 1}>
                <ChevronLeft className="icon-sm" />
                Previous
              </Button>
              <span className="subtitle-xs">
                {currentIssuePage}/{totalIssuePages}
              </span>
              <Button
                variant="outline"
                size="sm"
                aria-label="Next issues page"
                onClick={() => setIssuePage((page) => Math.min(totalIssuePages, page + 1))}
                disabled={currentIssuePage === totalIssuePages}>
                Next
                <ChevronRight className="icon-sm" />
              </Button>
            </div>
          ) : null}
        </>
      )}

      {selectedIssue ? (
        <CodeIssueDossier
          issue={selectedIssue}
          projectId={currentResult.projectId}
          scanUrl={currentResult.environmentUrl ?? ""}
          projectPath={projectPath}
          framework={currentResult.framework}
          onVerify={() => void handleVerifyIssue(selectedIssue)}
          verifying={verifyingIssueId === selectedIssue.id}
          onOpenTarget={onOpenTarget}
          onOpenIntegrations={onNavigate ? () => onNavigate("integrations") : undefined}
          onClose={() => setSelectedIssueId(null)}
        />
      ) : null}
    </div>
  );
}
