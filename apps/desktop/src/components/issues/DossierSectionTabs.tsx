import { useState, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

export interface DossierSectionTab {
  label: string;
  content: ReactNode;
}

/** Render non-empty dossier sections as tabs. */
export function DossierSectionTabs({ tabs }: { tabs: DossierSectionTab[] }) {
  const available = tabs.filter((tab) => Boolean(tab.content));
  const [active, setActive] = useState(0);

  if (available.length === 0) return null;
  const activeIndex = active < available.length ? active : 0;

  return (
    <div className="dossier-tabs">
      <div className="dossier-tab-bar" role="tablist">
        {available.map((tab, index) => (
          <Button
            unstyled
            key={tab.label}
            type="button"
            role="tab"
            aria-selected={index === activeIndex}
            onClick={() => setActive(index)}
            className={cn("dossier-tab", index === activeIndex && "dossier-tab-active")}>
            {tab.label}
          </Button>
        ))}
      </div>
      <div className="dossier-tab-panel">{available[activeIndex].content}</div>
    </div>
  );
}
