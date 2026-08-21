interface Props {
  pages: string[];
}

export function CrossPageBadge({ pages }: Props) {
  if (pages.length <= 1) return null;
  return <span className="affects-badge">Affects {pages.length} pages</span>;
}
