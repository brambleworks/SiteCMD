import { useState } from "react";
import { ShieldCheck } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { formatRelativeTime } from "@/lib/tokens";
import type { SiteBaseline, SiteBaselineField } from "@/generated/ipc-bindings";
import { useCurrentTime } from "@/lib/useCurrentTime";

interface Props {
  baseline: SiteBaseline | null;
  loading?: boolean;
  deciding?: boolean;
  refusal?: string | null;
  onDecide: (field: SiteBaselineField, accept: boolean) => void;
}

export function SiteBaselineCard({ baseline, loading, deciding, refusal, onDecide }: Props) {
  const nowMs = useCurrentTime();
  const [confirming, setConfirming] = useState<string | null>(null);

  if (loading) {
    return (
      <div className="card card-column">
        <BaselineTitle />
        <div className="baseline-list">
          {Array.from({ length: 2 }, (_, index) => (
            <div key={`baseline-skeleton-${index}`} className="baseline-row">
              <Skeleton className="baseline-skeleton-label" />
              <Skeleton className="baseline-skeleton-body" />
            </div>
          ))}
        </div>
      </div>
    );
  }

  if (!baseline || baseline.fields.length === 0) return null;

  return (
    <div className="card card-column">
      <BaselineTitle />
      {refusal ? (
        <p className="baseline-refusal" role="alert">
          {refusal}
        </p>
      ) : null}
      <div className="baseline-list">
        {baseline.fields.map((field) => (
          <BaselineRow
            key={field.field}
            field={field}
            nowMs={nowMs}
            deciding={Boolean(deciding)}
            confirming={confirming === field.field}
            onConfirm={() => setConfirming(field.field)}
            onCancel={() => setConfirming(null)}
            onDecide={(accept) => {
              setConfirming(null);
              onDecide(field, accept);
            }}
          />
        ))}
      </div>
    </div>
  );
}

function BaselineTitle() {
  return (
    <div className="card__title-rule">
      <span className="card__title">
        <ShieldCheck className="card__icon icon-md" aria-hidden="true" />
        <span>Baseline</span>
      </span>
      <p className="text-meta baseline-lead">
        What SiteCMD expects this site to keep doing. Confirm each line so changes stand out.
      </p>
    </div>
  );
}

function BaselineRow({
  field,
  nowMs,
  deciding,
  confirming,
  onConfirm,
  onCancel,
  onDecide,
}: {
  field: SiteBaselineField;
  nowMs: number;
  deciding: boolean;
  confirming: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  onDecide: (accept: boolean) => void;
}) {
  const changed = field.status !== "good";
  return (
    <div className="baseline-row">
      <div className="baseline-row-head">
        <h3 className="text-body baseline-row-label">{field.label}</h3>
        <span className="baseline-row-status">{statusText(field, nowMs)}</span>
      </div>

      {changed ? (
        <div className="baseline-compare">
          <BaselineValues heading="Recorded as good" lines={field.goodLines} />
          <BaselineValues heading="Seen now" lines={field.changedLines} emphasis />
        </div>
      ) : null}

      {changed ? (
        <div className="baseline-actions">
          {confirming ? (
            <>
              <p className="baseline-confirm-text">
                Accepting makes this the baseline for {field.label.toLowerCase()}. Later scans
                compare against it instead of the recorded value.
              </p>
              <Button variant="outline" size="sm" onClick={onCancel} disabled={deciding}>
                Cancel
              </Button>
              <Button size="sm" onClick={() => onDecide(true)} disabled={deciding}>
                Accept as baseline
              </Button>
            </>
          ) : (
            <>
              <Button variant="outline" size="sm" onClick={onConfirm} disabled={deciding}>
                Accept as baseline
              </Button>
              {field.status === "changed" && field.canDismiss ? (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => onDecide(false)}
                  disabled={deciding}>
                  Dismiss
                </Button>
              ) : null}
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}

function BaselineValues({
  heading,
  lines,
  emphasis,
}: {
  heading: string;
  lines: string[];
  emphasis?: boolean;
}) {
  return (
    <div className={emphasis ? "baseline-values baseline-values--changed" : "baseline-values"}>
      <p className="baseline-values-heading">{heading}</p>
      <ul className="baseline-values-list">
        {lines.length === 0 ? (
          <li className="baseline-value">Nothing recorded</li>
        ) : (
          lines.map((line) => (
            <li key={line} className="baseline-value">
              {line}
            </li>
          ))
        )}
      </ul>
    </div>
  );
}

/** One sentence per state, so a row never needs a legend to read. */
function statusText(field: SiteBaselineField, nowMs: number): string {
  if (field.status === "changed") {
    return `Changed ${formatRelativeTime(new Date(field.changeFirstSeenAt), nowMs)}`;
  }
  if (field.status === "silenced") {
    return "Change dismissed, baseline unchanged";
  }
  return `${field.origin} ${formatRelativeTime(new Date(field.recordedAt), nowMs)}`;
}
