import { useCallback, useEffect, useMemo, useState } from "react";
import { CheckCircle, ChevronRight } from "lucide-react";
import { Skeleton } from "@/components/ui/skeleton";
import type { IssueLink } from "@/lib/types";
import type { ProjectIssueSummary } from "@/lib/project-issue-summary";
import {
  formatSeverityLabel,
  isSeverity,
  severityCountTotal,
  severityToneClass,
} from "@/lib/severity";
import type { FixQueueSource, UnifiedFixIssue } from "@/lib/issue-ranking";
import { FilterSearch, FilterSelect } from "@/components/issues/IssueListFilters";
import { Button } from "@/components/ui/button";
import { Pager } from "@/components/ui/pager";
import {
  buildCategoryFilterCounts,
  buildCategoryOptions,
  filterIssuesByTitle,
  filterScanIssues,
  ISSUE_STATUS_LABELS,
  parseIssueCategoryFocus,
  parseSeverityFocus,
  SEVERITY_FILTER_LABELS,
  sortScanItems,
  type IssueStatusFilter,
  type SeverityFilter,
} from "@/components/issues/issue-list-model";
import type { IssueCategoryKey } from "@/lib/issue-categories";

export type { UnifiedFixIssue };
export type { IssueStatusFilter } from "@/components/issues/issue-list-model";

const ISSUE_PAGE_SIZE = 20;

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
  statusFilter,
  onStatusChange,
}: IssueListProps) {
  const [activeCategory, setActiveCategory] = useState<IssueCategoryKey | null>(null);
  const [search, setSearch] = useState("");
  const [activeSeverity, setActiveSeverity] = useState<SeverityFilter>("all");
  const [issuePage, setIssuePage] = useState(1);
  const showsScanQueue = !statusFilter || statusFilter === "active" || statusFilter === "all";

  const filtered = useMemo(
    () => filterScanIssues(rankedIssues, activeSeverity, activeCategory),
    [rankedIssues, activeCategory, activeSeverity],
  );

  const scanItems = useMemo(
    () => sortScanItems(filterIssuesByTitle(filtered, search)),
    [filtered, search],
  );
  const totalIssuePages = Math.max(1, Math.ceil(scanItems.length / ISSUE_PAGE_SIZE));
  const currentIssuePage = Math.min(issuePage, totalIssuePages);
  const issuePageStart = (currentIssuePage - 1) * ISSUE_PAGE_SIZE;
  const visibleScanItems = scanItems.slice(issuePageStart, issuePageStart + ISSUE_PAGE_SIZE);

  // Reset to the first page when the active filters change, adjusting state
  // during render instead of via an effect.
  const filterPageKey = `${activeCategory ?? ""}:${activeSeverity}:${statusFilter ?? ""}:${search}`;
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

  const categoryCounts = useMemo(() => buildCategoryFilterCounts(rankedIssues), [rankedIssues]);

  const categoryOptions = useMemo(() => buildCategoryOptions(categoryCounts), [categoryCounts]);

  useEffect(() => {
    const severityFocus = parseSeverityFocus(focus);
    if (severityFocus) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- derives the active filters from the focus prop; applies the initial focus on mount too
      setActiveSeverity(severityFocus);
      setActiveCategory(null);
      return;
    }
    setActiveCategory(parseIssueCategoryFocus(focus));
    setActiveSeverity("all");
  }, [focus]);

  useEffect(() => {
    if (!selectedId || scanItems.some((item) => item.id === selectedId)) return;
    onClearSelection?.();
  }, [onClearSelection, scanItems, selectedId]);

  const handleCategoryChange = useCallback((value: string) => {
    setActiveCategory(value === "all" ? null : (value as IssueCategoryKey));
  }, []);

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
            ariaLabel="Issue category"
            value={activeCategory ?? "all"}
            options={categoryOptions}
            onChange={handleCategoryChange}
          />
          <FilterSearch
            label="Search"
            ariaLabel="Search issue titles"
            placeholder="Search titles…"
            value={search}
            onChange={setSearch}
          />
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
                    {item.categoryLabel ? ` - ${item.categoryLabel}` : ""}
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

      {showsScanQueue ? (
        <Pager
          page={currentIssuePage}
          totalPages={totalIssuePages}
          onChange={setIssuePage}
          label="Issues pages"
          itemLabel="issues"
          className="subtle-divider-top issue-pager"
        />
      ) : null}
    </div>
  );
}

function IssueListLoadingContent({ statusFilter }: { statusFilter: IssueStatusFilter }) {
  const filterShells = [
    { label: "Status", value: ISSUE_STATUS_LABELS[statusFilter] },
    { label: "Severity", value: SEVERITY_FILTER_LABELS.all },
    { label: "Category", value: "All categories" },
    { label: "Search", value: "" },
  ];
  const loadingRows = [
    { severity: "Critical", category: "Security" },
    { severity: "High", category: "Database" },
    { severity: "Medium", category: "Performance" },
    { severity: "Low", category: "Accessibility" },
    { severity: "Critical", category: "Architecture" },
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
                  <span className="text-micro issue-loading-src-label">{row.category}</span>
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
