import { useState } from "react";
import { ArrowUpDown, Loader2, Minus, Radio, ScanSearch } from "lucide-react";
import { useJobs, type BackgroundJob, type BackgroundJobTarget } from "@/lib/jobs";
import { Button } from "@/components/ui/button";

const JOB_ICON: Record<BackgroundJob["type"], typeof ScanSearch> = {
  scan: ScanSearch,
  probes: Radio,
  sync: ArrowUpDown,
};

interface JobsTrayProps {
  onOpenJob?: (job: BackgroundJob) => void;
}

export function JobsTray({ onOpenJob }: JobsTrayProps) {
  // Subscribes to the jobs store here, at the leaf, so per-tick scan-progress
  // publishes repaint only this tray - never the app shell hosting it.
  const running = useJobs();
  const [minimized, setMinimized] = useState(false);

  if (running.length === 0) return null;

  if (minimized) {
    return (
      <div className="jobs-tray-trigger-wrap">
        <Button
          unstyled
          type="button"
          onClick={() => setMinimized(false)}
          className="jobs-tray-trigger"
          aria-label="Show running jobs">
          <Loader2 className="icon-sm animate-spin text-primary" />
          <span className="tabular-nums">{running.length}</span>
          <span>{running.length === 1 ? "job running" : "jobs running"}</span>
        </Button>
      </div>
    );
  }

  return (
    <div className="jobs-tray-stack">
      <div className="jobs-tray-panel">
        <div className="jobs-panel-header">
          <div className="flex-fill">
            <div className="text-micro jobs-tray-heading">Jobs</div>
            <p className="text-meta jobs-tray-count">{running.length} running</p>
          </div>
          <Button
            unstyled
            type="button"
            onClick={() => setMinimized(true)}
            className="jobs-tray-dismiss"
            aria-label="Minimize jobs">
            <Minus className="icon-sm" />
          </Button>
        </div>

        <div className="jobs-tray-scroll">
          <JobGroup jobs={running} onOpenJob={onOpenJob} />
        </div>
      </div>
    </div>
  );
}

function JobGroup({
  jobs,
  onOpenJob,
}: {
  jobs: BackgroundJob[];
  onOpenJob?: (job: BackgroundJob) => void;
}) {
  return (
    <div>
      <div className="stack-tight">
        {jobs.map((job) => (
          <JobRow key={job.id} job={job} onOpenJob={onOpenJob} />
        ))}
      </div>
    </div>
  );
}

function JobRow({
  job,
  onOpenJob,
}: {
  job: BackgroundJob;
  onOpenJob?: (job: BackgroundJob) => void;
}) {
  const RunningIcon = JOB_ICON[job.type] || Loader2;
  const isClickable = Boolean(onOpenJob && hasJobDestination(job.target));

  const content = (
    <>
      <RunningIcon className="icon-sm animate-pulse text-primary jobs-tray-row-icon" />
      <div className="flex-fill">
        <div className="text-truncate text-body-muted jobs-tray-row-label">{job.label}</div>
        {job.scopeLabel || job.detail ? (
          <div className="text-micro jobs-tray-row-meta">
            {[job.scopeLabel, job.detail].filter(Boolean).join(" · ")}
          </div>
        ) : null}
      </div>
      {job.progress != null ? (
        <span className="text-micro text-primary jobs-tray-row-pct">{job.progress}%</span>
      ) : null}
    </>
  );

  const className = "jobs-tray-row text-body";

  if (isClickable) {
    return (
      <Button
        unstyled
        type="button"
        onClick={() => onOpenJob?.(job)}
        className={`${className} jobs-tray-row--clickable`}>
        {content}
      </Button>
    );
  }

  return <div className={className}>{content}</div>;
}

function hasJobDestination(target: BackgroundJobTarget | null | undefined): boolean {
  if (!target) return false;
  return target.restoreScan === true || "page" in target;
}
