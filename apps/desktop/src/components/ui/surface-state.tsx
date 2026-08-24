import type { ReactNode } from "react";
import { AlertTriangle, Inbox } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button, type ButtonProps } from "@/components/ui/button";

type SurfaceStateKind = "empty" | "error";

interface SurfaceStateAction {
  label: string;
  onClick: () => void;
  variant?: ButtonProps["variant"];
}

interface SurfaceStateProps {
  kind: SurfaceStateKind;
  title: string;
  description: string;
  icon?: ReactNode;
  primaryAction?: SurfaceStateAction;
  secondaryAction?: SurfaceStateAction;
  className?: string;
  children?: ReactNode;
}

function getDefaultIcon(kind: SurfaceStateKind): ReactNode {
  switch (kind) {
    case "error":
      return <AlertTriangle className="empty-state-icon text-severity-medium" />;
    case "empty":
    default:
      return <Inbox className="empty-state-icon" />;
  }
}

export function SurfaceState({
  kind,
  title,
  description,
  icon,
  primaryAction,
  secondaryAction,
  className,
  children,
}: SurfaceStateProps) {
  return (
    <div
      className={cn("panel panel--empty", className)}
      role={kind === "error" ? "alert" : undefined}>
      {icon ?? getDefaultIcon(kind)}
      <p className="text-sm-bold">{title}</p>
      <p className="body-muted surface-state-desc">{description}</p>
      {(primaryAction || secondaryAction) && (
        <div className="surface-state-actions">
          {primaryAction ? (
            <Button size="sm" onClick={primaryAction.onClick} variant={primaryAction.variant}>
              {primaryAction.label}
            </Button>
          ) : null}
          {secondaryAction ? (
            <Button
              size="sm"
              variant={secondaryAction.variant ?? "ghost"}
              onClick={secondaryAction.onClick}>
              {secondaryAction.label}
            </Button>
          ) : null}
        </div>
      )}
      {children}
    </div>
  );
}
