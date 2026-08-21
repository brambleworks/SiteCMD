import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Clock, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import { getScanSchedule, saveScanSchedule } from "@/lib/commands";
import type { ScheduledScanType } from "@/lib/types";
import { queryKeys } from "@/lib/query/query-keys";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";

interface ScanScheduleCardProps {
  projectId?: number;
  environmentId?: number;
  projectPath?: string | null;
}

const DAYS_OF_WEEK = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

// Scheduled and manual runs use the same Full Scan composition.
const SCHEDULE_SCAN_TYPE: ScheduledScanType = "full";

export function ScanScheduleCard({ projectId, environmentId, projectPath }: ScanScheduleCardProps) {
  const toast = useToast();
  const queryClient = useQueryClient();
  const [schedOpen, setSchedOpen] = useState(false);
  const [schedFreq, setSchedFreq] = useState("off");
  const [schedTime, setSchedTime] = useState("09:00");
  const [schedDay, setSchedDay] = useState(1);
  const [schedNextRun, setSchedNextRun] = useState<string | null>(null);
  const [schedSaving, setSchedSaving] = useState(false);
  const scheduleQueryKey = queryKeys.settings.scanSchedule(projectId ?? 0, environmentId ?? 0);
  const scheduleQuery = useQuery({
    queryKey: scheduleQueryKey,
    queryFn: () =>
      getScanSchedule({
        projectId: projectId as number,
        environmentId: environmentId as number,
        scanType: SCHEDULE_SCAN_TYPE,
      }),
    enabled: projectId != null && environmentId != null,
  });
  const schedLoaded = !scheduleQuery.isPending && !scheduleQuery.isError;

  const hasLinkedProject = Boolean(projectPath?.trim());
  const codeInclusionNote = hasLinkedProject
    ? "Each run is a full scan: web checks plus a Code Scan of the linked project."
    : "Each run is a web scan. Link a project folder to include a Code Scan.";

  useResetOnChange(scheduleQuery.data, () => {
    const schedule = scheduleQuery.data;
    setSchedFreq(schedule?.frequency ?? "off");
    setSchedTime(schedule?.timeOfDay ?? "09:00");
    setSchedDay(schedule?.dayOfWeek ?? 1);
    setSchedNextRun(schedule?.nextRunAt ?? null);
  });

  const saveSchedule = async () => {
    if (!projectId || !environmentId) return false;
    setSchedSaving(true);
    try {
      const result = await saveScanSchedule({
        projectId,
        environmentId,
        frequency: schedFreq,
        timeOfDay: schedTime,
        dayOfWeek: schedFreq === "weekly" ? schedDay : null,
        scanType: SCHEDULE_SCAN_TYPE,
      });
      queryClient.setQueryData(scheduleQueryKey, result);
      setSchedNextRun(result.nextRunAt);
      toast.success("Schedule saved", "A full scan will run on this cadence.");
      return true;
    } catch {
      return false;
    } finally {
      setSchedSaving(false);
    }
  };

  if (!projectId || !environmentId) return null;

  return (
    <>
      <div className="schedule-card-wrapper scan-schedule-summary bg-card">
        <div className="scan-schedule-summary-row">
          <div className="scan-schedule-summary-copy">
            <div className="scan-schedule-summary-head">
              <Clock className="icon-md text-foreground" />
              <h3 className="text-body text-strong">Scheduled scans</h3>
            </div>
            {scheduleQuery.isPending ? (
              <LoadingRegion
                label="Scheduled scans loading state"
                className="scan-schedule-loading">
                <Skeleton variant="line" width="lg" />
                <Skeleton variant="line" width="lg" />
              </LoadingRegion>
            ) : scheduleQuery.isError ? (
              <p className="agent-handoff-error scan-schedule-error">
                Saved schedule settings could not load.
              </p>
            ) : (
              <p className="text-body-muted text-relaxed scan-schedule-desc">
                {schedFreq === "off"
                  ? `Run a full scan automatically on a daily or weekly cadence. ${codeInclusionNote}`
                  : `A full scan is set to run ${schedFreq} at ${schedTime}${schedFreq === "weekly" ? ` on ${DAYS_OF_WEEK[schedDay]}` : ""}${schedNextRun ? ` · Next ${new Date(schedNextRun).toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}` : ""}.`}
              </p>
            )}
          </div>
          {scheduleQuery.isError ? (
            <Button size="sm" variant="outline" onClick={() => void scheduleQuery.refetch()}>
              Retry
            </Button>
          ) : (
            <Button size="sm" onClick={() => setSchedOpen(true)} disabled={!schedLoaded}>
              {schedFreq === "off" ? "Set Up Schedule" : "Manage Schedule"}
            </Button>
          )}
        </div>
      </div>

      {schedOpen ? (
        <div
          className="overlay-backdrop scan-schedule-backdrop"
          role="dialog"
          aria-modal="true"
          aria-label="Scheduled scans"
          onClick={() => setSchedOpen(false)}>
          <div className="scan-schedule-panel" onClick={(event) => event.stopPropagation()}>
            <div className="scan-schedule-dialog-head">
              <div>
                <h2 className="scan-schedule-dialog-title text-foreground">Scheduled scans</h2>
                <p className="text-body-muted text-relaxed scan-schedule-desc">
                  {codeInclusionNote}
                </p>
              </div>
              <Button
                unstyled
                type="button"
                onClick={() => setSchedOpen(false)}
                className="icon-btn"
                aria-label="Close schedule dialog">
                <X className="icon-sm" />
              </Button>
            </div>

            <div className="scan-schedule-form">
              {schedLoaded ? (
                <>
                  <div className="scan-schedule-field">
                    <span className="form-label text-foreground">Frequency</span>
                    <div className="scan-schedule-freq">
                      {(["off", "daily", "weekly"] as const).map((frequency) => (
                        <Button
                          key={frequency}
                          size="sm"
                          variant={schedFreq === frequency ? "default" : "outline"}
                          onClick={() => setSchedFreq(frequency)}>
                          {frequency === "off" ? "Off" : frequency === "daily" ? "Daily" : "Weekly"}
                        </Button>
                      ))}
                    </div>
                  </div>

                  {schedFreq !== "off" ? (
                    <div className="scan-schedule-inline-field">
                      <span className="form-label text-foreground scan-schedule-inline-label">
                        Time
                      </span>
                      <input
                        type="time"
                        value={schedTime}
                        onChange={(event) => setSchedTime(event.target.value)}
                        className="field-control field-control--muted"
                      />
                    </div>
                  ) : null}

                  {schedFreq === "weekly" ? (
                    <div className="scan-schedule-inline-field">
                      <span className="form-label text-foreground scan-schedule-inline-label">
                        Day
                      </span>
                      <div className="scan-schedule-days">
                        {DAYS_OF_WEEK.map((dayLabel, index) => (
                          <Button
                            key={dayLabel}
                            size="sm"
                            variant={schedDay === index ? "default" : "outline"}
                            onClick={() => setSchedDay(index)}
                            className="scan-schedule-day-btn">
                            {dayLabel}
                          </Button>
                        ))}
                      </div>
                    </div>
                  ) : null}

                  <div className="scan-schedule-actions">
                    <Button
                      onClick={async () => {
                        const saved = await saveSchedule();
                        if (saved) {
                          setSchedOpen(false);
                        }
                      }}
                      disabled={schedSaving}>
                      {schedSaving ? "Saving..." : "Save Schedule"}
                    </Button>
                    <Button variant="ghost" onClick={() => setSchedOpen(false)}>
                      Cancel
                    </Button>
                  </div>
                </>
              ) : (
                <div className="surface-low-panel">Loading saved schedule settings...</div>
              )}
            </div>
          </div>
        </div>
      ) : null}
    </>
  );
}
