import { ChevronRight } from "lucide-react";
import type {
  ConnectedAlert,
  ConnectedAlertDelivery,
  ConnectedDestination,
} from "@/generated/ipc-bindings-connected";
import { Button } from "@/components/ui/button";
import {
  DossierNumberedSection,
  DossierRail,
  IssueDossierPanel,
} from "@/components/issues/IssueDossierPanel";
import type { NavTarget } from "@/components/layout/nav-page";
import {
  alertSeverityLabel,
  alertSeverityToneClass,
  alertTitle,
  causeLabel,
  formatAlertTimestamp,
  outcomeLabel,
  outcomeToneClass,
  targetKindLabel,
} from "./connected-alert-display";

interface ConnectedAlertDossierProps {
  alert: ConnectedAlert;
  destinations: ConnectedDestination[];
  onClose: () => void;
  onNavigate?: (page: NavTarget) => void;
}

/** Omits a hosted link because only the emailed one-time nonce can authenticate it. */
export function ConnectedAlertDossier({
  alert,
  destinations,
  onClose,
  onNavigate,
}: ConnectedAlertDossierProps) {
  const leftRail = (
    <DossierRail className="dossier-rail-section-plain">
      <div className="dossier-rail-list">
        <div className="dossier-rail-row">
          <span className="dossier-rail-row-key">Raised</span>
          <span className="dossier-rail-row-value">{formatAlertTimestamp(alert.raisedAt)}</span>
        </div>
        <div className="dossier-rail-row">
          <span className="dossier-rail-row-key">Last changed</span>
          <span className="dossier-rail-row-value">{formatAlertTimestamp(alert.updatedAt)}</span>
        </div>
        {alert.deploymentId ? (
          <div className="dossier-rail-row">
            <span className="dossier-rail-row-key">Deployment</span>
            <span className="dossier-rail-row-value">{alert.deploymentId}</span>
          </div>
        ) : null}
      </div>
    </DossierRail>
  );

  const rightRail = onNavigate ? (
    <DossierRail>
      <div className="dossier-rail-button-stack">
        <Button onClick={() => onNavigate("issues")} aria-label="Open Issues">
          <ChevronRight className="icon-md" />
          <span>Open Issues</span>
        </Button>
      </div>
    </DossierRail>
  ) : undefined;

  return (
    <IssueDossierPanel
      title={alertTitle(alert)}
      subtitle="Raised by the connected service, not by a scan on this machine."
      eyebrow={
        <>
          <span className={alertSeverityToneClass(alert.severity)}>
            {alertSeverityLabel(alert.severity)}
          </span>
          {" - Connected service"}
        </>
      }
      leftRail={leftRail}
      rightRail={rightRail}
      onClose={onClose}>
      <DossierNumberedSection label="What The Service Found" tone="attention">
        {alert.causes.length === 0 ? (
          <p className="body-text">
            The service raised this alert without recording a cause line, which happens for
            conditions it watches outside a scan.
          </p>
        ) : (
          <ul className="connected-alert-cause-list">
            {alert.causes.map((cause) => (
              <li
                key={`${cause.kind}-${cause.severity ?? "none"}`}
                className="connected-alert-fact">
                <span className="connected-alert-fact__label">{causeLabel(cause.kind)}</span>
                <span className={`eyebrow ${alertSeverityToneClass(cause.severity)}`}>
                  {alertSeverityLabel(cause.severity)}
                </span>
                <span className="text-meta">
                  {cause.count === 1 ? "1 finding" : `${cause.count} findings`}
                </span>
              </li>
            ))}
          </ul>
        )}
        <p className="text-body-muted">
          Counts and classes only. The service never holds the evidence behind a finding, which is
          why the detail is in Issues on this machine and not in the message it sent.
        </p>
      </DossierNumberedSection>

      <DossierNumberedSection label="Who Was Told" tone="verify">
        {alert.delivery.length === 0 ? (
          <p className="body-text">
            Nobody. This site has no confirmed alert address and no webhook endpoint, so the service
            recorded the alert and sent it nowhere.
          </p>
        ) : (
          <ul className="connected-alert-delivery-list">
            {alert.delivery.map((cell) => (
              <li
                key={`${cell.targetKind}-${cell.targetId}`}
                className="connected-alert-fact connected-alert-fact--delivery">
                <span className={outcomeToneClass(cell.outcome)} />
                <span className="connected-alert-fact__label">
                  {deliveryTargetLabel(cell, destinations)}
                </span>
                <span className="text-meta">{outcomeLabel(cell.outcome)}</span>
              </li>
            ))}
          </ul>
        )}
      </DossierNumberedSection>
    </IssueDossierPanel>
  );
}

// Non-admin reads may omit destination addresses; do not expose opaque ids.
function deliveryTargetLabel(
  cell: ConnectedAlertDelivery,
  destinations: ConnectedDestination[],
): string {
  const kind = targetKindLabel(cell.targetKind);
  if (cell.targetKind !== "destination") return kind;
  const address = destinations.find(
    (destination) => destination.destinationId === cell.targetId,
  )?.address;
  return address ? `${kind}: ${address}` : kind;
}
