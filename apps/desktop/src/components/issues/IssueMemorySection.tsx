import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getEvents, getIssueCheckMemory, getProjects } from "@/lib/commands";
import { MS_PER_DAY } from "@/lib/format";
import { isActionableCheckStatus } from "@/lib/issues";
import { DossierRail } from "@/components/issues/IssueDossierPanel";
import { Button } from "@/components/ui/button";
import { InlineSkeleton, LoadingRegion } from "@/components/ui/skeleton";
import { queryKeys } from "@/lib/query/query-keys";

interface IssueMemorySectionProps {
  projectId: number;
  url: string;
  checkId: string;
  currentStatus?: string | null;
}

interface IssueMemorySnapshot {
  firstSeen: number | null;
  lastFailed: number | null;
  lastVerified: number | null;
  regressedAfterDeploy: { occurredAtMs: number; title: string } | null;
  affectedEnvironments: string[];
}

function formatTimestamp(timestamp: number | null): string {
  if (!timestamp) return "-";
  return new Date(timestamp).toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function useIssueMemory({ projectId, checkId, currentStatus }: IssueMemorySectionProps) {
  const queryClient = useQueryClient();
  const memoryQuery = useQuery<IssueMemorySnapshot>({
    queryKey: queryKeys.issueMemory.forCheck(projectId, checkId, currentStatus ?? null),
    queryFn: async () => {
      const [projects, lifecycle] = await Promise.all([
        queryClient.ensureQueryData({
          queryKey: queryKeys.projects.list(),
          queryFn: getProjects,
        }),
        getIssueCheckMemory({ projectId, checkId }),
      ]);

      const envs = projects.find((entry) => entry.id === projectId)?.environments ?? [];
      const labelByUrl = new Map(envs.map((env) => [env.url, env.label]));
      const affectedEnvironments = lifecycle.affectedEnvUrls.map(
        (envUrl) => labelByUrl.get(envUrl) ?? envUrl,
      );

      const { firstSeen, lastFailed, lastVerified } = lifecycle;

      let regressedAfterDeploy: { occurredAtMs: number; title: string } | null = null;
      if (isActionableCheckStatus(currentStatus) && lastFailed) {
        const endMs = Date.now();
        const startMs = endMs - 180 * MS_PER_DAY;
        const deployEvents = await getEvents({
          projectId,
          startMs,
          endMs,
          eventTypes: ["deploy"],
        });
        regressedAfterDeploy =
          (Array.isArray(deployEvents) ? deployEvents : [])
            .filter((event) => {
              const verifiedAt = lastVerified ?? Number.NEGATIVE_INFINITY;
              return event.occurredAtMs <= lastFailed && event.occurredAtMs > verifiedAt;
            })
            .sort((a, b) => b.occurredAtMs - a.occurredAtMs)
            .map((event) => ({ occurredAtMs: event.occurredAtMs, title: event.title }))
            .at(0) ?? null;
      }

      return {
        firstSeen,
        lastFailed,
        lastVerified,
        regressedAfterDeploy,
        affectedEnvironments,
      };
    },
  });

  return {
    loading: memoryQuery.isPending,
    memory: memoryQuery.data ?? null,
    error: memoryQuery.isError,
    retry: () => void memoryQuery.refetch(),
  };
}

export function IssueMemoryRail(props: IssueMemorySectionProps) {
  const { loading, memory, error, retry } = useIssueMemory(props);

  return (
    <DossierRail className="dossier-rail-section-plain">
      {loading ? (
        <LoadingRegion label="Loading issue history" className="dossier-rail-list">
          {[0, 1, 2].map((index) => (
            <div key={index} className="dossier-rail-row">
              <InlineSkeleton className="issue-memory-skeleton-key" />
              <InlineSkeleton className="issue-memory-skeleton-value" />
            </div>
          ))}
        </LoadingRegion>
      ) : error ? (
        <div className="dossier-rail-list">
          <p className="body-muted">Issue history could not load.</p>
          <Button size="sm" variant="outline" onClick={retry}>
            Retry
          </Button>
        </div>
      ) : !memory ? (
        <p className="body-muted">No issue memory yet.</p>
      ) : (
        <div className="dossier-rail-list">
          <div className="dossier-rail-row">
            <span className="dossier-rail-row-key">First seen</span>
            <span className="dossier-rail-row-value">{formatTimestamp(memory.firstSeen)}</span>
          </div>
          <div className="dossier-rail-row">
            <span className="dossier-rail-row-key">Last failed</span>
            <span className="dossier-rail-row-value">{formatTimestamp(memory.lastFailed)}</span>
          </div>
          {memory.lastVerified ? (
            <div className="dossier-rail-row">
              <span className="dossier-rail-row-key">Verified</span>
              <span className="dossier-rail-row-value text-emerald-300">
                {formatTimestamp(memory.lastVerified)}
              </span>
            </div>
          ) : null}
          {memory.regressedAfterDeploy ? (
            <p className="text-meta text-amber-300 text-relaxed">
              Regressed after {memory.regressedAfterDeploy.title}.
            </p>
          ) : null}
          {memory.affectedEnvironments.length > 0 ? (
            <div className="dossier-rail-row">
              <p className="dossier-rail-label">Environments</p>
              <div className="dossier-rail-env-list">
                {memory.affectedEnvironments.map((label) => (
                  <span key={label} className="dossier-rail-row-value">
                    {label}
                  </span>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      )}
    </DossierRail>
  );
}
