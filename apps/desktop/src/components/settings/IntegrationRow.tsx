import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";

interface IntegrationRowProps {
  icon: ReactNode;
  name: string;
  /** Connected and healthy: shows a green dot beside the title. */
  connected?: boolean;
  /** Right-aligned action affordance, e.g. "Set up" or "Manage". */
  actionLabel: string;
  disabled?: boolean;
  onOpen: () => void;
  /** Deep-link focus target. */
  dataIntegration?: string;
}

export function IntegrationRow({
  icon,
  name,
  connected = false,
  actionLabel,
  disabled = false,
  onOpen,
  dataIntegration,
}: IntegrationRowProps) {
  return (
    <Button
      unstyled
      type="button"
      onClick={onOpen}
      disabled={disabled}
      data-integration={dataIntegration}
      className="list-row list-row--interactive integration-row">
      {icon}
      <span className="row flex-fill">
        <span className="integration-row__name">{name}</span>
        {connected ? (
          <span className="status-dot-success" role="img" aria-label="Connected" />
        ) : null}
      </span>
      <span className="integration-row__action">{actionLabel}</span>
    </Button>
  );
}
