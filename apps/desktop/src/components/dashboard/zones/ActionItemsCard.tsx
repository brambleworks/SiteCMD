import { ListChecks, RefreshCw } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { CompactTrendSparkline } from "@/components/dashboard/CompactTrend";
import type { CompactTrendModel } from "@/components/dashboard/compact-trend-model";

type DashboardPrimaryActionKey = "issues" | "updates";

export interface DashboardPrimaryActionCard {
  key: DashboardPrimaryActionKey;
  label: string;
  value: string;
  detail: string;
  trend: CompactTrendModel;
  onClick: () => void;
}

export interface ActionItemsData {
  cards: DashboardPrimaryActionCard[];
}

interface Props {
  items: ActionItemsData;
}

const PRIMARY_ACTION_ICON: Record<DashboardPrimaryActionKey, LucideIcon> = {
  issues: ListChecks,
  updates: RefreshCw,
};

export function ActionItemsCard({ items }: Props) {
  if (items.cards.length === 0) return null;
  return (
    <div className="action-items-grid">
      {items.cards.map((card) => (
        <PrimaryActionCard key={card.key} card={card} />
      ))}
    </div>
  );
}

function PrimaryActionCard({ card }: { card: DashboardPrimaryActionCard }) {
  const Icon = PRIMARY_ACTION_ICON[card.key];
  return (
    <Button
      unstyled
      type="button"
      onClick={card.onClick}
      className="card card--interactive primary-action-card">
      <div className="card__title-rule">
        <span className="card__title">
          <Icon className="card__icon icon-md" aria-hidden="true" />
          <span>{card.label}</span>
        </span>
        <span className={`text-meta action-items-delta ${getTrendToneClass(card.trend)}`}>
          {card.trend.deltaLabel}
        </span>
      </div>
      <div>
        <p className="action-card__value">{card.value}</p>
        <p className="action-card__detail">{card.detail}</p>
      </div>
      <div className="action-items-spark">
        <CompactTrendSparkline model={card.trend} height={44} />
      </div>
    </Button>
  );
}

function getTrendToneClass(model: CompactTrendModel): string {
  if (model.tone === "improving") return "text-score-excellent";
  if (model.tone === "worsening") return "text-severity-critical";
  if (model.tone === "stable") return "text-primary";
  return "text-muted-foreground";
}
