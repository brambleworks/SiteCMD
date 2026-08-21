import type { TransitiveCause } from "@/lib/types";

interface Props {
  causes: TransitiveCause[];
}

export function TransitiveCauseCallout({ causes }: Props) {
  if (causes.length === 0) return null;
  const deepest = causes.reduce((a, b) => (b.depth > a.depth ? b : a));
  const chain = deepest.path.slice().reverse().join(" → ");
  return (
    <div className="callout-root-cause">
      <div>
        <div className="callout-root-cause-title">Root-cause chain</div>
        <div className="callout-root-cause-body">{chain}</div>
        <div className="callout-root-cause-body">Fix the upstream issue to break the chain.</div>
      </div>
    </div>
  );
}
