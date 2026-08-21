import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";

export function IssuePanelSkeleton({
  label,
  rows = 5,
  className = "panel-inset",
}: {
  label: string;
  rows?: number;
  className?: string;
}) {
  return (
    <LoadingRegion label={label} className={`${className} stack-base`}>
      <div className="row-between">
        <div className="stack-snug">
          <Skeleton className="issue-skeleton-title" />
          <Skeleton className="issue-skeleton-subtitle" />
        </div>
        <Skeleton className="issue-skeleton-badge" />
      </div>
      <div className="panel panel--flush panel--muted">
        {Array.from({ length: rows }, (_, index) => (
          <div key={index} className="list-row">
            <Skeleton className="issue-skeleton-avatar" />
            <div className="flex-fill stack-snug">
              <Skeleton className="issue-skeleton-line" />
              <Skeleton className="issue-skeleton-subline" />
            </div>
            <Skeleton className="issue-skeleton-row-badge" />
          </div>
        ))}
      </div>
    </LoadingRegion>
  );
}
