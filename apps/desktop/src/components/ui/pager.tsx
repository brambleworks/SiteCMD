import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { buildPagerItems } from "@/lib/pagination";
import { cn } from "@/lib/utils";

interface PagerProps {
  page: number;
  totalPages: number;
  onChange: (page: number) => void;
  /** Nav landmark name, for example "Issues pages". */
  label: string;
  /** Noun used in the step labels, for example "issues". */
  itemLabel: string;
  className?: string;
}

/** Page links with steps either side. Steps are absent, not disabled, at the ends. */
export function Pager({ page, totalPages, onChange, label, itemLabel, className }: PagerProps) {
  if (totalPages <= 1) return null;

  return (
    <nav aria-label={label} className={cn("pager", className)}>
      {page > 1 ? (
        <Button
          unstyled
          type="button"
          className="pager-step"
          aria-label={`Previous ${itemLabel} page`}
          onClick={() => onChange(page - 1)}>
          <ChevronLeft className="icon-xs" />
          Previous
        </Button>
      ) : null}

      <div className="pager-pages">
        {buildPagerItems(page, totalPages).map((item, index) =>
          item === "gap" ? (
            <span key={`gap-${index}`} className="pager-gap" aria-hidden="true">
              …
            </span>
          ) : (
            <Button
              unstyled
              key={item}
              type="button"
              className={cn("pager-page", item === page && "pager-page--current")}
              aria-label={`Page ${item}`}
              aria-current={item === page ? "page" : undefined}
              onClick={() => onChange(item)}>
              {item}
            </Button>
          ),
        )}
      </div>

      {page < totalPages ? (
        <Button
          unstyled
          type="button"
          className="pager-step"
          aria-label={`Next ${itemLabel} page`}
          onClick={() => onChange(page + 1)}>
          Next
          <ChevronRight className="icon-xs" />
        </Button>
      ) : null}
    </nav>
  );
}
