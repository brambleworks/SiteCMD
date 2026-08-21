interface Props {
  count: number;
}

export function DownstreamBadge({ count }: Props) {
  if (count === 0) return null;
  return <span className="downstream-badge">+{count} downstream</span>;
}
