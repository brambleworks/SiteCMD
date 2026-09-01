export type PagerItem = number | "gap";

const WINDOW_LIMIT = 7;

/**
 * Page numbers to render in a pager: the first page, the last page, and the
 * pages either side of the current one, with gaps standing in for the rest.
 * Short ranges list every page.
 */
export function buildPagerItems(page: number, totalPages: number): PagerItem[] {
  if (totalPages <= WINDOW_LIMIT) {
    return Array.from({ length: Math.max(0, totalPages) }, (_, index) => index + 1);
  }

  const candidates = new Set([1, totalPages, page - 1, page, page + 1]);
  const visible = [...candidates]
    .filter((value) => value >= 1 && value <= totalPages)
    .sort((left, right) => left - right);

  const items: PagerItem[] = [];
  let previous = 0;
  for (const value of visible) {
    if (previous && value - previous > 1) items.push("gap");
    items.push(value);
    previous = value;
  }
  return items;
}
