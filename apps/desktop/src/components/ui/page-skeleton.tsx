import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";

export type PageSkeletonLayout = "dashboard" | "data" | "split" | "timeline" | "cards";

interface PageSkeletonProps {
  label: string;
  layout: PageSkeletonLayout;
}

function SkeletonRows({ count = 5 }: { count?: number }) {
  return (
    <div className="skeleton-page-list">
      {Array.from({ length: count }, (_, index) => (
        <div key={index} className="skeleton-page-row">
          <Skeleton variant="avatar" />
          <div className="skeleton-lines">
            <Skeleton variant="line-lg" width="wide" />
            <Skeleton variant="line" width="wide" />
          </div>
          <Skeleton variant="badge" />
        </div>
      ))}
    </div>
  );
}

function SkeletonStats({ count }: { count: number }) {
  return (
    <div className="skeleton-page-stats">
      {Array.from({ length: count }, (_, index) => (
        <div key={index} className="stat-card">
          <Skeleton variant="line" width="sm" />
          <Skeleton variant="stat" width="xs" />
          <Skeleton variant="line" width="sm" />
        </div>
      ))}
    </div>
  );
}

function SkeletonCards() {
  return (
    <div className="skeleton-page-card-grid">
      {[0, 1, 2, 3].map((index) => (
        <div key={index} className="card card--spacious skeleton-stack">
          <div className="row-between">
            <Skeleton variant="avatar-lg" />
            <Skeleton variant="badge" />
          </div>
          <Skeleton variant="line-lg" width="md" />
          <Skeleton variant="line" width="full" />
          <Skeleton variant="line" width="wide" />
          <Skeleton variant="button" width="full" />
        </div>
      ))}
    </div>
  );
}

/** Preserve page density and columns while a lazy route loads. */
export function PageSkeleton({ label, layout }: PageSkeletonProps) {
  return (
    <LoadingRegion label={label} className="skeleton-page">
      <div className="skeleton-page-toolbar">
        <div className="skeleton-stack">
          <Skeleton variant="title" width="md" />
          <Skeleton variant="line" width="lg" />
        </div>
        <Skeleton variant="button" width="sm" />
      </div>

      {layout === "dashboard" ? (
        <>
          <SkeletonStats count={6} />
          <div className="skeleton-page-split">
            <div className="skeleton-page-panel">
              <SkeletonRows count={4} />
            </div>
            <div className="skeleton-page-panel">
              <SkeletonRows count={4} />
            </div>
          </div>
        </>
      ) : null}

      {layout === "data" ? (
        <>
          <SkeletonStats count={3} />
          <div className="skeleton-page-panel">
            <SkeletonRows />
          </div>
        </>
      ) : null}

      {layout === "split" ? (
        <div className="skeleton-page-split">
          <div className="skeleton-page-panel">
            <SkeletonRows count={6} />
          </div>
          <div className="skeleton-page-panel skeleton-page-detail">
            <Skeleton variant="heading" width="wide" />
            <Skeleton variant="line-lg" width="narrow" />
            <Skeleton variant="block" width="full" />
            <SkeletonRows count={3} />
          </div>
        </div>
      ) : null}

      {layout === "timeline" ? (
        <div className="skeleton-page-panel">
          <SkeletonRows count={7} />
        </div>
      ) : null}

      {layout === "cards" ? <SkeletonCards /> : null}
    </LoadingRegion>
  );
}
