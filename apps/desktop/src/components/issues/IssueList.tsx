import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, CheckCircle, ChevronLeft, ChevronRight, Copy } from "lucide-react";
import { copyToClipboard } from "@/lib/clipboard";
import { Skeleton } from "@/components/ui/skeleton";
import { buildBatchFixPrompt } from "@/lib/fix-copilot-batch";
import { recordWorkflowHealthEvent } from "@/lib/observability";
import type { CodeScanDomain, IssueLink, ScanCategory } from "@/lib/types";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import {
  formatSeverityLabel,
  isSeverity,
  severityCountTotal,
  severityToneClass,
} from "@/lib/severity";
import type { FixQueueSource, UnifiedFixIssue } from "@/lib/issue-ranking";
import { FilterSelect } from "@/components/issues/IssueListFilters";
import { Button } from "@/components/ui/button";
import {
  buildBatchFixItems,
  buildCodeFilterCounts,
  buildSubfilterOptions,
  buildWebFilterCounts,
  filterScanIssues,
  getActiveSubfilterValue,
  ISSUE_SOURCE_LABELS,
  ISSUE_STATUS_LABELS,
  parseIssueFilterFocus,
  parseIssueSourceFocus,
  parseSeverityFocus,
  SEVERITY_FILTER_LABELS,
  sortScanItems,
  type IssueFilter,
  type IssueSourceFilter,
  type IssueStatusFilter,
  type SeverityFilter,
} from "@/components/issues/issue-list-model";

export type { UnifiedFixIssue };
export type { IssueStatusFilter } from "@/components/issues/issue-list-model";

const ISSUE_PAGE_SIZE = 100;

interface IssueListProps {
  // Ranked from the canonical IssueGroup projection by the owning page so the
  // list and dossier cause-navigation share one identity/order pass.
  rankedIssues: UnifiedFixIssue[];
  loading?: boolean;
  issueLinks: IssueLink[];
  issueSummary: ProjectIssueSummary;
  selectedId: string | null;
  focus?: string | null;
  onSelect: (item: UnifiedFixIssue) => void;
  onClearSelection?: () => void;
  hideControls?: boolean;
  url?: string;
  detectedStack?: Record<string, unknown> | null;
  statusFilter?: IssueStatusFilter;
  onStatusChange?: (next: IssueStatusFilter) => void;
}

export function IssueList({
  rankedIssues,
  loading = false,
  issueLinks,
  issueSummary,
  selectedId,
  onSelect,
  focus = null,
  onClearSelection,
  hideControls = false,
  url,
  detectedStack,
  statusFilter,
  onStatusChange,
}: IssueListProps) {
  const [batchCopied, setBatchCopied] = useState(false);
  const [activeSource, setActiveSource] = useState<IssueSourceFilter>("all");
  const [activeFilter, setActiveFilter] = useState<IssueFilter | null>(null);
  const [activeSeverity, setActiveSeverity] = useState<SeverityFilter>("all");
  const [issuePage, setIssuePage] = useState(1);
  const showsScanQueue = !statusFilter || statusFilter === "active" || statusFilter === "all";

  const filtered = useMemo(
    () => filterScanIssues(rankedIssues, activeSource, activeSeverity, activeFilter),
    [rankedIssues, activeFilter, activeSource, activeSeverity],
  );

  const scanItems = useMemo(() => sortScanItems(filtered), [filtered]);
  const totalIssuePages = Math.max(1, Math.ceil(scanItems.length / ISSUE_PAGE_SIZE));
  const currentIssuePage = Math.min(issuePage, totalIssuePages);
  const issuePageStart = (currentIssuePage - 1) * ISSUE_PAGE_SIZE;
  const visibleScanItems = scanItems.slice(issuePageStart, issuePageStart + ISSUE_PAGE_SIZE);

  // Reset to the first page when the active filters change, adjusting state
  // during render instead of via an effect.
  const filterPageKey = `${activeFilter ?? ""}:${activeSeverity}:${activeSource}:${statusFilter ?? ""}`;
  const [pagedFilterKey, setPagedFilterKey] = useState(filterPageKey);
  if (pagedFilterKey !== filterPageKey) {
    setPagedFilterKey(filterPageKey);
    setIssuePage(1);
  }

  const severityCounts = issueSummary.severityCounts;

  const linkMap = useMemo(() => {
    const map = new Map<string, IssueLink>();
    for (const link of issueLinks) {
      if (!map.has(link.checkId) || link.status === "open") map.set(link.checkId, link);
    }
    return map;
  }, [issueLinks]);

  const webCount = issueSummary.webCount;
  const codeCount = issueSummary.codeCount;

  const webFilterCounts = useMemo(() => buildWebFilterCounts(rankedIssues), [rankedIssues]);
  const codeFilterCounts = useMemo(() => buildCodeFilterCounts(rankedIssues), [rankedIssues]);

  const subfilterOptions = useMemo(() => {
    return buildSubfilterOptions({
      activeSource,
      webCount,
      codeCount,
      webFilterCounts,
      codeFilterCounts,
    });
  }, [activeSource, codeCount, codeFilterCounts, webCount, webFilterCounts]);

  const activeSubfilterValue = useMemo(() => getActiveSubfilterValue(activeFilter), [activeFilter]);

  useEffect(() => {
    const severityFocus = parseSeverityFocus(focus);
    if (severityFocus) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- derives the active filters from the focus prop; applies the initial focus on mount too
      setActiveSeverity(severityFocus);
      setActiveFilter(null);
      setActiveSource("all");
      return;
    }
    const sourceFocus = parseIssueSourceFocus(focus);
    if (sourceFocus) {
      setActiveSource(sourceFocus);
      setActiveFilter(null);
      setActiveSeverity("all");
      return;
    }
    const nextFilter = parseIssueFilterFocus(focus);
    setActiveFilter(nextFilter);
    setActiveSeverity("all");
    if (nextFilter?.kind === "web-category") {
      setActiveSource("web");
      return;
    }
    if (nextFilter?.kind === "code-domain") {
      setActiveSource("code");
      return;
    }
    setActiveSource("all");
  }, [focus]);

  useEffect(() => {
    if (!selectedId || scanItems.some((item) => item.id === selectedId)) return;
    onClearSelection?.();
  }, [onClearSelection, scanItems, selectedId]);

  const handleSourceChange = useCallback((value: string) => {
    const nextSource: IssueSourceFilter = value === "web" || value === "code" ? value : "all";
    setActiveSource(nextSource);
    setActiveFilter((current) => {
      if (!current) return null;
      if (nextSource === "web" && current.kind === "web-category") return current;
      if (nextSource === "code" && current.kind === "code-domain") return current;
      return null;
    });
  }, []);

  const handleSubfilterChange = useCallback(
    (value: string) => {
      if (value === "all" || activeSource === "all") {
        setActiveFilter(null);
        return;
      }
      if (activeSource === "web" && value.startsWith("web:")) {
        setActiveFilter({ kind: "web-category", category: value.slice(4) as ScanCategory });
        return;
      }
      if (activeSource === "code" && value.startsWith("code:")) {
        setActiveFilter({ kind: "code-domain", domain: value.slice(5) as CodeScanDomain });
        return;
      }
      setActiveFilter(null);
    },
    [activeSource],
  );

  const handleCopyBatch = useCallback(async () => {
    if (scanItems.length === 0) return;
    recordWorkflowHealthEvent("copy_guidance", "started", {
      source: "batch_prompt",
      issueCount: scanItems.length,
    });
    const items = buildBatchFixItems(scanItems);
    const prompt = buildBatchFixPrompt(items, { url, detectedStack });
    const ok = await copyToClipboard(prompt);
    recordWorkflowHealthEvent("copy_guidance", ok ? "succeeded" : "failed", {
      source: "batch_prompt",
      issueCount: scanItems.length,
    });
    if (ok) {
      setBatchCopied(true);
      setTimeout(() => setBatchCopied(false), 2000);
    }
  }, [scanItems, url, detectedStack]);

  if (loading) {
    return <IssueListLoadingContent statusFilter={statusFilter ?? "active"} />;
  }

  if (showsScanQueue && rankedIssues.length === 0) {
    return (
      <div className="issue-empty-row">
        <CheckCircle className="icon-lg text-score-excellent" />
        <div>
          <p className="row-title-md">No web or code issues open</p>
          <p className="text-meta text-foreground">No issues to fix right now.</p>
        </div>
      </div>
    );
  }

  return (
    <div>
      {!hideControls ? (
        <div className="subtle-toolbar-row">
          {onStatusChange ? (
            <FilterSelect
              label="Status"
              ariaLabel="Issue status"
              value={statusFilter ?? "active"}
              options={(Object.keys(ISSUE_STATUS_LABELS) as IssueStatusFilter[]).map((s) => ({
                value: s,
                label: ISSUE_STATUS_LABELS[s],
              }))}
              onChange={(value) => onStatusChange(value as IssueStatusFilter)}
            />
          ) : null}
          <FilterSelect
            label="Show"
            ariaLabel="Issue source"
            value={activeSource}
            options={[
              { value: "all", label: `${ISSUE_SOURCE_LABELS.all} (${webCount + codeCount})` },
              { value: "web", label: `${ISSUE_SOURCE_LABELS.web} (${webCount})` },
              { value: "code", label: `${ISSUE_SOURCE_LABELS.code} (${codeCount})` },
            ]}
            onChange={handleSourceChange}
          />
          <FilterSelect
            label="Severity"
            ariaLabel="Issue severity"
            value={activeSeverity}
            options={[
              {
                value: "all",
                label: `${SEVERITY_FILTER_LABELS.all} (${severityCountTotal(severityCounts)})`,
              },
              {
                value: "critical",
                label: `${SEVERITY_FILTER_LABELS.critical} (${severityCounts.critical})`,
              },
              { value: "high", label: `${SEVERITY_FILTER_LABELS.high} (${severityCounts.high})` },
              {
                value: "medium",
                label: `${SEVERITY_FILTER_LABELS.medium} (${severityCounts.medium})`,
              },
              { value: "low", label: `${SEVERITY_FILTER_LABELS.low} (${severityCounts.low})` },
            ]}
            onChange={(value) => setActiveSeverity(value as SeverityFilter)}
          />
          <FilterSelect
            label="Category"
            ariaLabel="Issue subcategory"
            value={activeSubfilterValue}
            options={subfilterOptions}
            onChange={handleSubfilterChange}
            disabled={activeSource === "all"}
          />
          {scanItems.length > 0 ? (
            <Button
              unstyled
              type="button"
              onClick={handleCopyBatch}
              className="queue-tool-button issue-toolbar-trailing"
              title={`Copy a single AI prompt covering all ${scanItems.length} visible issues`}>
              {batchCopied ? (
                <Check className="icon-xs text-score-excellent" />
              ) : (
                <Copy className="icon-xs" />
              )}
              <span>{batchCopied ? "Copied" : "Batch prompt"}</span>
            </Button>
          ) : null}
        </div>
      ) : null}

      {showsScanQueue && scanItems.length === 0 && rankedIssues.length > 0 ? (
        <div className="issue-no-match text-muted-foreground">No issues match this filter yet.</div>
      ) : null}

      {showsScanQueue &&
        visibleScanItems.map((item, index) => {
          const isSelected = item.id === selectedId;

          const link = item.kind === "web" ? linkMap.get(item.issue.checkId) : undefined;
          const title = item.issue.title;
          const severityLabel = formatSeverityLabel(item.issue.severity);
          const severityClass = isSeverity(item.issue.severity)
            ? severityToneClass(item.issue.severity)
            : sourceTextClass(item.kind);
          return (
            <Button
              unstyled
              key={item.id}
              type="button"
              data-dossier-switch="true"
              onClick={() => onSelect(item)}
              className={`list-row list-row--issue subtle-divider-bottom issue-row ${isSelected ? "issue-row--selected" : ""}`}>
              <div className="issue-row-lead">
                <span className="mono-subtle issue-row-num">{issuePageStart + index + 1}.</span>
                <div className="issue-row-text">
                  <div className="text-micro issue-row-meta">
                    <span className={severityClass}>{severityLabel}</span>
                    {item.sourceLabel ? ` - ${item.sourceLabel}` : ""}
                  </div>
                  <div className="list-row__title text-body issue-row-title">{title}</div>
                </div>
              </div>
              <div className="issue-row-trail">
                {link && <span className="text-mono-xs text-foreground">{link.externalId}</span>}
                <ChevronRight
                  className={`list-row__chevron icon-md ${severityClass}`}
                  aria-hidden="true"
                />
              </div>
            </Button>
          );
        })}

      {showsScanQueue && totalIssuePages > 1 ? (
        <div className="row-between subtle-divider-top issue-pager">
          <Button
            variant="outline"
            size="sm"
            aria-label="Previous issues page"
            onClick={() => setIssuePage((page) => Math.max(1, page - 1))}
            disabled={currentIssuePage === 1}>
            <ChevronLeft />
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
            <ChevronRight />
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function IssueListLoadingContent({ statusFilter }: { statusFilter: IssueStatusFilter }) {
  const filterShells = [
    { label: "Status", value: ISSUE_STATUS_LABELS[statusFilter] },
    { label: "Show", value: ISSUE_SOURCE_LABELS.all },
    { label: "Severity", value: SEVERITY_FILTER_LABELS.all },
    { label: "Category", value: "All subcategories" },
  ];
  const loadingRows = [
    { severity: "Critical", source: "Web scan" },
    { severity: "High", source: "Code scan" },
    { severity: "Medium", source: "Web scan" },
    { severity: "Low", source: "Code scan" },
    { severity: "Critical", source: "Web scan" },
  ];

  return (
    <div aria-label="Issues data loading state" aria-busy="true">
      <div className="subtle-toolbar-row">
        {filterShells.map((filter) => (
          <div key={filter.label} className="issue-filter-shell-group">
            <div className="eyebrow issue-filter-label text-muted-foreground">{filter.label}</div>
            <div className="filter-shell-display">
              <span>{filter.value}</span>
              <Skeleton variant="line" width="xs" />
            </div>
          </div>
        ))}
      </div>

      <div>
        {loadingRows.map((row, index) => (
          <div key={index} className="row-between subtle-divider-bottom issue-row">
            <div className="issue-row-lead">
              <span className="mono-subtle issue-row-num">{index + 1}.</span>
              <div className="issue-loading-body">
                <div className={`text-micro issue-row-meta ${loadingSeverityClass(row.severity)}`}>
                  {row.severity}
                </div>
                <Skeleton variant="line-lg" width="lg" />
                <div className="issue-loading-src">
                  <span className="text-micro issue-loading-src-label">{row.source}</span>
                  <span className="text-micro issue-loading-dot">·</span>
                  <Skeleton variant="line" width="md" />
                </div>
              </div>
            </div>
            <div className="issue-row-trail">
              <Skeleton variant="line" width="xs" />
              <Skeleton variant="line" width="xs" />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function sourceTextClass(source: FixQueueSource): string {
  if (source === "code") return "text-cat-code";
  if (source === "alert") return "text-severity-high";
  return "text-primary";
}

function loadingSeverityClass(label: string): string {
  const severity = label.toLowerCase();
  if (isSeverity(severity)) return severityToneClass(severity);
  return "text-primary";
}
