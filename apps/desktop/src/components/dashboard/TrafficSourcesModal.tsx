import { useEffect } from "react";
import { createPortal } from "react-dom";
import { Activity, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { NavTarget } from "@/components/layout/nav-page";

interface TrafficSource {
  label: string;
  integrationType: string;
  status: string;
  statusLabel: string;
}

interface TrafficProviderError {
  label: string;
  integrationType: string;
  message: string;
}

interface TrafficSourcesModalProps {
  sources: TrafficSource[];
  providerErrors: TrafficProviderError[];
  onNavigate?: (page: NavTarget) => void;
  onClose: () => void;
}

export function TrafficSourcesModal({
  sources,
  providerErrors,
  onNavigate,
  onClose,
}: TrafficSourcesModalProps) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [onClose]);

  const goToIntegration = (integrationType: string) => {
    onClose();
    onNavigate?.(`integrations:${integrationType}`);
  };

  return createPortal(
    <div className="fix-prompt-modal-backdrop" onClick={onClose}>
      <section
        className="fix-prompt-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="traffic-sources-title"
        onClick={(e) => e.stopPropagation()}>
        <div className="fix-prompt-modal-header">
          <h3 id="traffic-sources-title" className="fix-prompt-modal-title">
            Traffic sources
          </h3>
          <Button
            unstyled
            type="button"
            className="details-close"
            aria-label="Close"
            onClick={onClose}>
            <X />
          </Button>
        </div>

        <div className="agent-handoff-body">
          <p className="body-muted">
            Where this page&apos;s traffic data comes from. Select a source to manage its setup.
          </p>

          <div className="traffic-sources-list">
            {sources.map((source) => (
              <Button
                unstyled
                key={source.integrationType}
                type="button"
                aria-label={`${source.label}: ${source.statusLabel}. Open integration settings.`}
                className={`traffic-source-button traffic-source-button--${source.status} btn--block`}
                onClick={() => goToIntegration(source.integrationType)}>
                <span className="traffic-source-button__dot" aria-hidden="true" />
                <span>{source.label}</span>
                <span className="traffic-source-button__status">{source.statusLabel}</span>
              </Button>
            ))}
          </div>

          {providerErrors.map((providerError) => (
            <div className="traffic-control-warning" key={providerError.integrationType}>
              <Activity className="icon-sm" aria-hidden="true" />
              <p className="text-body text-relaxed">
                {providerError.label}: {providerError.message}
              </p>
              {onNavigate ? (
                <Button
                  variant="outline"
                  size="sm"
                  className="traffic-warning-action"
                  onClick={() => goToIntegration(providerError.integrationType)}>
                  Fix setup
                </Button>
              ) : null}
            </div>
          ))}
        </div>

        <div className="fix-prompt-modal-footer">
          <Button variant="outline" type="button" onClick={onClose}>
            Close
          </Button>
        </div>
      </section>
    </div>,
    document.body,
  );
}
