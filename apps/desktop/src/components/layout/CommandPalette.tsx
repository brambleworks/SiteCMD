import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import { Button } from "@/components/ui/button";
import {
  Search,
  LayoutDashboard,
  CalendarDays,
  BarChart3,
  Layers,
  Globe,
  GitBranch,
  RefreshCw,
  FileText,
  Plug,
  Settings,
  RotateCcw,
  Plus,
  ArrowRight,
} from "lucide-react";
import { ACTION_SHORTCUTS, PAGE_SHORTCUTS } from "@/app/keyboard-shortcuts";
import type { NavTarget } from "@/components/layout/nav-page";

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  onNavigate: (page: NavTarget) => void;
  onAction?: (action: string) => void;
}

interface CommandItem {
  id: string;
  label: string;
  category: string;
  icon: typeof Search;
  action: () => void;
  keywords?: string[];
  /** Display label for the keyboard shortcut that runs this command, if any. */
  shortcut?: string;
}

export function CommandPalette({ open, onClose, onNavigate, onAction }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const items: CommandItem[] = useMemo(
    () => [
      {
        id: "sites",
        label: "Overview",
        category: "Pages",
        icon: Layers,
        action: () => onNavigate("sites"),
        keywords: ["sites", "projects", "all", "switch", "overview", "scores", "issues"],
      },
      {
        id: "dashboard",
        label: "Dashboard",
        category: "Pages",
        icon: LayoutDashboard,
        action: () => onNavigate("dashboard"),
        keywords: ["home", "overview"],
        shortcut: PAGE_SHORTCUTS.dashboard?.label,
      },
      {
        id: "issues",
        label: "Issues",
        category: "Pages",
        icon: Search,
        action: () => onNavigate("issues"),
        shortcut: PAGE_SHORTCUTS.issues?.label,
        keywords: [
          "issues",
          "list",
          "results",
          "web",
          "code",
          "history",
          "compare",
          "fix",
          "security",
          "vulnerabilities",
          "threats",
        ],
      },
      {
        id: "integrations",
        label: "Integrations",
        category: "Pages",
        icon: Plug,
        action: () => onNavigate("integrations"),
        keywords: [
          "connect",
          "sources",
          "services",
          "api",
          "plausible",
          "cloudflare",
          "github",
          "uptime",
          "code review",
          "security",
        ],
      },
      {
        id: "search",
        label: "Search & SEO",
        category: "Pages",
        icon: Globe,
        action: () => onNavigate("search-console"),
        shortcut: PAGE_SHORTCUTS["search-console"]?.label,
        keywords: ["google", "bing", "seo", "rankings", "discoverability", "search visibility"],
      },
      {
        id: "updates",
        label: "Updates",
        category: "Pages",
        icon: RefreshCw,
        action: () => onNavigate("updates"),
        shortcut: PAGE_SHORTCUTS.updates?.label,
        keywords: ["npm", "packages", "dependencies"],
      },
      {
        id: "deploys",
        label: "Deploys",
        category: "Pages",
        icon: GitBranch,
        action: () => onNavigate("deploys"),
        shortcut: PAGE_SHORTCUTS.deploys?.label,
        keywords: ["github", "commits", "releases"],
      },
      {
        id: "analytics",
        label: "Traffic",
        category: "Pages",
        icon: BarChart3,
        action: () => onNavigate("analytics"),
        shortcut: PAGE_SHORTCUTS.analytics?.label,
        keywords: ["traffic", "visitors", "pageviews", "uptime", "cdn", "performance"],
      },
      {
        id: "events",
        label: "Activity",
        category: "Pages",
        icon: CalendarDays,
        action: () => onNavigate("events"),
        shortcut: PAGE_SHORTCUTS.events?.label,
        keywords: ["events", "activity", "calendar", "timeline"],
      },
      {
        id: "reports",
        label: "Reports",
        category: "Pages",
        icon: FileText,
        action: () => onNavigate("reports"),
        keywords: ["pdf", "export", "report"],
      },
      {
        id: "settings",
        label: "Settings",
        category: "Pages",
        icon: Settings,
        action: () => onNavigate("settings"),
        shortcut: PAGE_SHORTCUTS.settings?.label,
        keywords: ["preferences", "config", "account"],
      },
      {
        id: "run-scan",
        label: "Run Scan",
        category: "Actions",
        icon: RotateCcw,
        action: () => onAction?.("scan"),
        shortcut: ACTION_SHORTCUTS.runScan.label,
        keywords: ["check", "test", "audit"],
      },
      {
        id: "add-project",
        label: "Add Project",
        category: "Actions",
        icon: Plus,
        action: () => onAction?.("add-project"),
        shortcut: ACTION_SHORTCUTS.addProject.label,
        keywords: ["new", "create", "site"],
      },
    ],
    [onNavigate, onAction],
  );

  const filtered = useMemo(() => {
    if (!query.trim()) return items;
    const q = query.toLowerCase();
    return items.filter(
      (item) =>
        item.label.toLowerCase().includes(q) ||
        item.category.toLowerCase().includes(q) ||
        item.keywords?.some((k) => k.includes(q)),
    );
  }, [query, items]);

  const grouped = useMemo(() => {
    const map = new Map<string, CommandItem[]>();
    for (const item of filtered) {
      const existing = map.get(item.category) || [];
      existing.push(item);
      map.set(item.category, existing);
    }
    return map;
  }, [filtered]);

  useEffect(() => {
    if (open) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- resets the palette and focuses the input when it opens; the focus call is imperative
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [open]);

  // Keep the selected index in range, adjusting state during render instead of
  // via an effect. React bails out when the value is unchanged.
  if (selectedIndex >= filtered.length) setSelectedIndex(Math.max(0, filtered.length - 1));

  useEffect(() => {
    const el = listRef.current?.querySelector(`[data-index="${selectedIndex}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const execute = useCallback(
    (item: CommandItem) => {
      onClose();
      item.action();
    },
    [onClose],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter" && filtered[selectedIndex]) {
        e.preventDefault();
        execute(filtered[selectedIndex]);
      } else if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    },
    [filtered, selectedIndex, execute, onClose],
  );

  if (!open) return null;

  let flatIndex = -1;

  return (
    <div className="overlay-backdrop overlay-backdrop--command" onClick={onClose}>
      <div className="command-backdrop-scrim" />
      <div className="command-palette-panel" onClick={(e) => e.stopPropagation()}>
        <div className="command-search-row">
          <Search className="icon-muted" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelectedIndex(0);
            }}
            onKeyDown={handleKeyDown}
            placeholder="Search pages and actions…"
            className="command-search-input"
            autoComplete="off"
            spellCheck={false}
          />
          <kbd className="keyboard-hint">ESC</kbd>
        </div>

        <div ref={listRef} className="command-results">
          {filtered.length === 0 ? (
            <div className="command-no-results">No results for &ldquo;{query}&rdquo;</div>
          ) : (
            Array.from(grouped.entries()).map(([category, categoryItems]) => (
              <div key={category}>
                <div className="command-group-label">{category}</div>
                {categoryItems.map((item) => {
                  flatIndex++;
                  const idx = flatIndex;
                  const Icon = item.icon;
                  return (
                    <Button
                      unstyled
                      key={item.id}
                      data-index={idx}
                      onClick={() => execute(item)}
                      onMouseEnter={() => setSelectedIndex(idx)}
                      className={`command-item ${
                        idx === selectedIndex ? "command-item--selected" : "command-item--idle"
                      }`}>
                      <Icon className="icon-muted" />
                      <span className="command-item-label">{item.label}</span>
                      {item.shortcut ? (
                        <kbd className="command-shortcut">{item.shortcut}</kbd>
                      ) : null}
                      <ArrowRight className="icon-xs command-item-arrow" />
                    </Button>
                  );
                })}
              </div>
            ))
          )}
        </div>

        <div className="command-footer">
          <span>
            <kbd>↑↓</kbd> navigate
          </span>
          <span>
            <kbd>↵</kbd> select
          </span>
          <span>
            <kbd>esc</kbd> close
          </span>
        </div>
      </div>
    </div>
  );
}
