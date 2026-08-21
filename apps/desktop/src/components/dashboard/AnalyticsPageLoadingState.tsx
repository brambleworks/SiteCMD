import { PageSkeleton } from "@/components/ui/page-skeleton";

export function AnalyticsLoadingState({ syncing = false }: { syncing?: boolean }) {
  return (
    <PageSkeleton
      label={syncing ? "Analytics loading state - finishing setup" : "Analytics loading state"}
      layout="dashboard"
    />
  );
}
