export type PagerItem = number | "gap";

export interface PageWindow<T> {
  /** The requested page clamped into the range the rows actually cover. */
  page: number;
  totalPages: number;
  rows: T[];
}

/**
 * One page of rows plus the page numbers a Pager needs. Lists that can hold
 * thousands of entries render through this so the mounted row count stays at
 * the page size and the Pager reveals the rest.
 */
export function pageWindow<T>(rows: readonly T[], page: number, pageSize: number): PageWindow<T> {
  const totalPages = Math.max(1, Math.ceil(rows.length / pageSize));
  const currentPage = Math.min(Math.max(page, 1), totalPages);
  const start = (currentPage - 1) * pageSize;
  return { page: currentPage, totalPages, rows: rows.slice(start, start + pageSize) };
}

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
