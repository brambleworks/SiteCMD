import { AlertTriangle } from "lucide-react";
import type { LikelyCause } from "@/lib/types";
import { Button } from "@/components/ui/button";

interface Props {
  causes: LikelyCause[];
  onOpenCause: (checkId: string) => void;
}

export function RootCauseCallout({ causes, onOpenCause }: Props) {
  if (causes.length === 0) return null;

  return (
    <div className="callout-root-cause">
      <AlertTriangle className="callout-root-cause-icon icon-sm text-severity-medium" />
      <div>
        <div className="callout-root-cause-title">Likely root cause</div>
        <div className="callout-root-cause-body">
          {causes.map((c) => (
            <div key={c.checkId}>
              {c.confidence === "high" ? "Likely caused by" : "May be caused by"}{" "}
              <Button
                unstyled
                type="button"
                className="callout-root-cause-action"
                onClick={() => onOpenCause(c.checkId)}>
                {c.checkId}
              </Button>
              .
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
