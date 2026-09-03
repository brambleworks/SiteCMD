import { Search } from "lucide-react";
import { Button } from "@/components/ui/button";

/** Search trigger pill for the top bar. Opens the lazy command palette. */
export function SearchTrigger({ onClick }: { onClick: () => void }) {
  return (
    <Button unstyled onClick={onClick} className="command-search-trigger">
      <Search className="icon-sm" />
      <span className="text-truncate">Search pages and actions…</span>
      <kbd className="command-shortcut">⌘K</kbd>
    </Button>
  );
}
