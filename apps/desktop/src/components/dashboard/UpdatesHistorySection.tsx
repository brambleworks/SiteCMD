import { Skeleton } from "@/components/ui/skeleton";
import { ChevronDown, Clock3 } from "lucide-react";
import type { SiteEvent } from "@/lib/types";
import {
  formatHistoryTimestamp,
  getAppliedUpdateHistoryRows,
  getUpdateHistoryTitle,
} from "./update-history";

export function UpdatesHistorySection({
  events,
  loading,
}: {
  events: SiteEvent[];
  loading: boolean;
}) {
  return (
    <div className="stack-base">
      <div className="card__title-rule">
        <span className="card__title">
          <Clock3 className="card__icon icon-md" aria-hidden="true" />
          <span>History</span>
        </span>
      </div>

      <div className="panel panel--flush">
        {loading ? (
          <div className="update-history-loading">
            {[0, 1, 2].map((index) => (
              <div key={index} className="stack-snug">
                <Skeleton className="update-history-skeleton-title" />
                <Skeleton className="update-history-skeleton-line" />
                <Skeleton className="update-history-skeleton-line-short" />
              </div>
            ))}
          </div>
        ) : (
          <div className="divide-rows">
            {events.map((event) => {
              const detail = event.parsedDetail ?? null;
              const rows = getAppliedUpdateHistoryRows(detail);
              return (
                <details key={event.id} className="update-history-item">
                  <summary className="update-history-trigger">
                    <ChevronDown className="icon-sm update-history-chevron" aria-hidden="true" />
                    <div className="flex-fill">
                      <p className="row-title">{getUpdateHistoryTitle(event, rows)}</p>
                    </div>
                    <p className="subtitle-xs update-history-time">
                      {formatHistoryTimestamp(event.occurredAtMs)}
                    </p>
                  </summary>
                  {rows.length > 0 ? (
                    <div className="update-history-detail">
                      <div className="update-history-table-wrap ghost-border">
                        <table className="update-history-table">
                          <thead className="update-history-thead">
                            <tr>
                              <th>Package</th>
                              <th>Previous</th>
                              <th>New</th>
                            </tr>
                          </thead>
                          <tbody className="update-history-tbody">
                            {rows.map((row) => (
                              <tr
                                key={`${event.id}:${row.name}:${row.fromVersion}:${row.toVersion}`}>
                                <td>{row.name}</td>
                                <td>{row.fromVersion}</td>
                                <td>{row.toVersion}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    </div>
                  ) : null}
                </details>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
