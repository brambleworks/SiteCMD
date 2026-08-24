import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { ArrowLeft, ChevronRight, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { severityToneClass } from "@/lib/severity";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";

type BadgeTone = "default" | "critical" | "warning" | "success" | "info" | "muted";

export interface IssueDossierBadge {
  label: string;
  tone?: BadgeTone;
}

interface IssueDossierMetaItem {
  label: string;
  value: ReactNode;
  mono?: boolean;
}

type DossierSectionTone = "neutral" | "attention" | "action" | "verify" | "ai" | "supporting";

interface IssueDossierPanelProps {
  title: string;
  subtitle?: string;
  eyebrow?: ReactNode;
  eyebrowClassName?: string;
  badges?: IssueDossierBadge[];
  meta?: IssueDossierMetaItem[];
  leftRail?: ReactNode;
  rightRail?: ReactNode;
  headerActions?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  onClose: () => void;
  onBack?: () => void;
}

const BADGE_STYLES: Record<BadgeTone, string> = {
  default: "text-foreground",
  critical: severityToneClass("critical"),
  warning: severityToneClass("high"),
  success: "dossier-badge--success",
  info: "dossier-badge--info",
  muted: "text-muted-foreground",
};
const DOSSIER_TONE_CLASS: Record<DossierSectionTone, string> = {
  neutral: "dossier-section-tone-neutral",
  attention: "dossier-section-tone-attention",
  action: "dossier-section-tone-action",
  verify: "dossier-section-tone-verify",
  ai: "dossier-section-tone-ai",
  supporting: "dossier-section-tone-supporting",
};

function formatValue(value: unknown): string {
  if (value == null || value === "") return "-";
  if (Array.isArray(value)) return value.map(formatValue).join(", ");
  if (typeof value === "object") {
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  }
  return String(value);
}

export function IssueDossierPanel({
  title,
  subtitle,
  eyebrow = "Details",
  eyebrowClassName,
  badges = [],
  meta,
  leftRail,
  rightRail,
  headerActions,
  footer,
  children,
  onClose,
  onBack,
}: IssueDossierPanelProps) {
  const [visible, setVisible] = useState(false);
  const closeTimerRef = useRef<number | null>(null);
  const onCloseRef = useRef(onClose);
  const hasRails = Boolean(leftRail || rightRail);

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  const requestClose = useCallback(() => {
    setVisible(false);
    if (closeTimerRef.current) window.clearTimeout(closeTimerRef.current);
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null;
      onCloseRef.current();
    }, 180);
  }, []);

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => setVisible(true));
    // Dialog owns Escape and the top layer; this listener only replaces the
    // native backdrop-click dismissal (dismissOnBackdrop is false on the
    // Dialog below, since a click anywhere outside the panel closes it, not
    // only a click on this dialog's own backdrop).
    const onPointerDown = (event: PointerEvent) => {
      const targetNode = event.target instanceof Node ? event.target : null;
      const targetElement =
        event.target instanceof Element ? event.target : (targetNode?.parentElement ?? null);
      if (!targetElement) return;
      const nearestDialog = targetElement.closest("dialog");
      if (nearestDialog && !nearestDialog.querySelector(".details-panel")) {
        // The click landed on or inside a different native dialog (a handoff
        // modal opened from within this dossier, for example). That dialog
        // fills the viewport and sits on top; it is never an outside click.
        return;
      }
      if (!targetElement.closest(".details-panel")) {
        // A click on this dossier's own backdrop, as opposed to some other
        // outside element: never let it reach the app shell underneath.
        if (targetElement instanceof HTMLDialogElement) {
          event.stopPropagation();
        }
        requestClose();
      }
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("pointerdown", onPointerDown, true);
      if (closeTimerRef.current) window.clearTimeout(closeTimerRef.current);
    };
  }, [requestClose]);

  return (
    <Dialog
      label={title}
      onClose={requestClose}
      dismissOnBackdrop={false}
      className={cn("details-panel", visible ? "details-panel-visible" : "details-panel-hidden")}>
      <div className="details-header">
        <div className="dossier-header-row">
          <div className="dossier-header-main">
            {onBack ? (
              <Button
                unstyled
                type="button"
                onClick={onBack}
                aria-label="Back to previous issue"
                className="details-back">
                <ArrowLeft className="icon-md" />
              </Button>
            ) : null}
            <div className="dossier-header-text">
              <p className={cn("details-eyebrow", eyebrowClassName)}>{eyebrow}</p>
              <h2 className="details-title">{title}</h2>
              {subtitle ? <p className="details-subtitle">{subtitle}</p> : null}
              {badges.length > 0 ? (
                <div className="dossier-badge-row">
                  {badges.map((badge) => (
                    <span
                      key={badge.label}
                      className={cn("dossier-badge", BADGE_STYLES[badge.tone ?? "default"])}>
                      {badge.label}
                    </span>
                  ))}
                </div>
              ) : null}
            </div>
          </div>

          {meta && meta.length > 0 ? (
            <div className="dossier-header-side">
              <div className="dossier-meta-grid">
                {meta.map((item) => (
                  <div key={item.label}>
                    <p className="dossier-meta-label">{item.label}</p>
                    <p className={cn("dossier-meta-value", item.mono && "font-mono")}>
                      {item.value}
                    </p>
                  </div>
                ))}
              </div>
              {headerActions ? (
                <div className="dossier-header-actions-slot">{headerActions}</div>
              ) : null}
            </div>
          ) : headerActions ? (
            <div className="dossier-header-side dossier-header-side-actions-only">
              <div className="dossier-header-actions-slot">{headerActions}</div>
            </div>
          ) : null}

          <Button
            unstyled
            type="button"
            onClick={requestClose}
            aria-label="Close details panel"
            className="details-close">
            <X />
          </Button>
        </div>
      </div>

      <div className={cn("details-body", hasRails && "details-body-railed")}>
        {leftRail ? <aside className="dossier-left-rail">{leftRail}</aside> : null}
        <div className="dossier-center">
          <div className="dossier-section-stack">{children}</div>
        </div>
        {rightRail ? <aside className="dossier-right-rail">{rightRail}</aside> : null}
      </div>

      {footer ? <div className="details-footer">{footer}</div> : null}
    </Dialog>
  );
}

export function DossierSection({
  label,
  action,
  children,
  tone = "neutral",
}: {
  label: string;
  action?: ReactNode;
  children: ReactNode;
  tone?: DossierSectionTone;
}) {
  return (
    <section className={cn("details-section", DOSSIER_TONE_CLASS[tone])}>
      <div className="details-section-header">
        <p className="details-section-label">{label}</p>
        {action}
      </div>
      {children}
    </section>
  );
}

export function DossierKeyValueGrid({
  data,
  limit = 8,
}: {
  data: Record<string, unknown>;
  limit?: number;
}) {
  const entries = Object.entries(data)
    .filter(([, value]) => value != null && value !== "")
    .slice(0, limit);

  if (entries.length === 0) {
    return <p className="muted-text">No extra evidence captured for this issue.</p>;
  }

  return (
    <div className="dossier-kv-grid">
      {entries.map(([key, value]) => (
        <div key={key} className="dossier-kv-cell">
          <p className="details-section-label">{key.replace(/_/g, " ")}</p>
          <p className="dossier-kv-value">{formatValue(value)}</p>
        </div>
      ))}
    </div>
  );
}

export function DossierNumberedSection({
  label,
  children,
  tone = "neutral",
}: {
  label: string;
  children: ReactNode;
  tone?: DossierSectionTone;
}) {
  return (
    <section className={cn("dossier-numbered-section", DOSSIER_TONE_CLASS[tone])}>
      <div className="dossier-numbered-header">
        <ChevronRight className="dossier-numbered-caret" />
        <span className="dossier-numbered-label">{label}</span>
      </div>
      <div className="dossier-numbered-body">{children}</div>
    </section>
  );
}

export function DossierRail({
  label,
  children,
  className,
}: {
  label?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={cn("dossier-rail-section", className)}>
      {label ? <h4 className="dossier-rail-label">{label}</h4> : null}
      <div className="dossier-rail-body">{children}</div>
    </section>
  );
}
