import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { completeGoogleOauth, connectGoogle, saveGoogleIntegration } from "@/lib/commands";
import { useToast } from "@/hooks/useToast";
import { invalidateProjectMonitoringSignals } from "@/lib/project-summary-signals";
import { getHostname } from "@/lib/utils";
import type { IntegrationConfig, IntegrationData } from "./integration-services";
import { hasSetupError, isIntegrationActive } from "./integration-connection-status";
import { IntegrationLiveDataPanel, IntegrationServiceIconBadge } from "./IntegrationServicePanels";
import { IntegrationModal } from "./IntegrationModal";
import { GooglePicker } from "./IntegrationGooglePicker";
import { Button } from "@/components/ui/button";
import { GoogleSignInButton } from "@/components/ui/google-sign-in-button";
import { ExtLink } from "@/components/ui/external-link";
import {
  filterGooglePickerData,
  googleChoiceCount,
  googleIntegrationLabel,
  isGoogleIntegrationType,
  pickPreferredGoogleChoice,
  type GoogleIntegrationType,
  type GooglePickerData,
} from "./google-integration-selection";

interface GoogleConnectService {
  type: GoogleIntegrationType;
  name: string;
}

type GoogleSetupError = {
  type: GoogleIntegrationType;
  message: string;
};

function googleCompletionErrorMessage() {
  return "Google returned the browser authorization, but SiteCMD could not finish setup.";
}

interface GoogleIntegrationConnectProps {
  service: GoogleConnectService | null;
  configs: IntegrationConfig[];
  liveData: Record<string, IntegrationData>;
  loadingData: Record<string, boolean>;
  projectId: number;
  url?: string;
  onFetchData: (type: string) => void;
  onDisconnect: (type: string) => void;
  onReloadConfigs: () => Promise<void>;
  onModalServiceChange: (type: string | null) => void;
}

/** Stays mounted across the OAuth grant and the page-level property picker. */
export function GoogleIntegrationConnect({
  service,
  configs,
  liveData,
  loadingData,
  projectId,
  url,
  onFetchData,
  onDisconnect,
  onReloadConfigs,
  onModalServiceChange,
}: GoogleIntegrationConnectProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [googleConnecting, setGoogleConnecting] = useState(false);
  const [googlePickerData, setGooglePickerData] = useState<GooglePickerData | null>(null);
  const [googleFlowId, setGoogleFlowId] = useState<string | null>(null);
  const [googlePickerTarget, setGooglePickerTarget] = useState<GoogleIntegrationType | null>(null);
  const [googleSetupError, setGoogleSetupError] = useState<GoogleSetupError | null>(null);

  const connectedTypes = new Set(configs.map((c) => c.integrationType));

  const saveGoogleConnection = async (
    flowId: string,
    type: GoogleIntegrationType,
    siteIdVal: string,
  ) => {
    await saveGoogleIntegration({
      projectId,
      flowId,
      integrationType: type,
      siteId: siteIdVal,
    });
    invalidateProjectMonitoringSignals(queryClient, projectId, url ?? null);
    setGooglePickerData(null);
    onModalServiceChange(null);
    setGoogleFlowId(null);
    setGooglePickerTarget(null);
    setGoogleSetupError(null);
    await onReloadConfigs();
    onFetchData(type);
    toast.success("Connected", `${googleIntegrationLabel(type)} is now active.`);
  };

  const handleGoogleConnect = async (requestedType?: string) => {
    const target = isGoogleIntegrationType(requestedType) ? requestedType : null;
    setGoogleConnecting(true);
    setGooglePickerData(null);
    setGoogleFlowId(null);
    setGooglePickerTarget(target);
    setGoogleSetupError((current) => (target && current?.type === target ? null : current));
    try {
      const started = await connectGoogle<{ flow_id: string }>({ projectId });
      setGoogleFlowId(started.flow_id);
      const data = await completeGoogleOauth<GooglePickerData>({
        projectId,
        flowId: started.flow_id,
      });

      // Single-service path: fast-connect if there is exactly one preferred choice.
      const projectHost = url ? getHostname(url) : "";
      const preferredChoice = target ? pickPreferredGoogleChoice(data, target, projectHost) : null;

      if (target && preferredChoice) {
        await saveGoogleConnection(started.flow_id, target, preferredChoice);
        return;
      }

      if (target) {
        if (googleChoiceCount(data, target) === 0) {
          // No sites for this service: keep the connect modal open with the error.
          onModalServiceChange(target);
          const backendError = target === "googleanalytics" ? data.ga4_error : data.gsc_error;
          const errorMessage = backendError
            ? `Could not load your Google data: ${backendError}`
            : `No ${googleIntegrationLabel(target)} sites were returned for this Google account.`;
          setGoogleSetupError({ type: target, message: errorMessage });
          toast.error(
            `${googleIntegrationLabel(target)} was not found`,
            "Make sure this Google account has access, then reconnect.",
          );
          setGooglePickerData(null);
          return;
        }
        // Multiple sites: show the picker and close the connect modal.
        onModalServiceChange(null);
        setGooglePickerData(filterGooglePickerData(data, target));
        return;
      }
      setGooglePickerData(filterGooglePickerData(data, target));
    } catch {
      const message = googleCompletionErrorMessage();
      if (target) {
        onModalServiceChange(target);
        setGoogleSetupError({ type: target, message });
        setGooglePickerTarget(target);
      } else {
        setGooglePickerTarget(null);
      }
      setGoogleFlowId(null);
      toast.error("Google setup did not finish", message);
    } finally {
      setGoogleConnecting(false);
    }
  };

  const handlePickGoogleProperty = async (type: string, siteIdVal: string) => {
    if (
      !googleFlowId ||
      !isGoogleIntegrationType(type) ||
      (googlePickerTarget !== null && type !== googlePickerTarget)
    ) {
      toast.error("Connection expired", "Reconnect Google and try again.");
      return;
    }
    try {
      await saveGoogleConnection(googleFlowId, type, siteIdVal);
    } catch {
      setGoogleSetupError({
        type,
        message: "SiteCMD could not save this Google selection.",
      });
      toast.error("Google setup did not finish", "SiteCMD could not save this Google selection.");
    }
  };

  const closeModal = () => {
    if (!service) return;
    onModalServiceChange(null);
    if (googlePickerTarget === service.type) {
      setGooglePickerData(null);
      setGoogleFlowId(null);
      setGooglePickerTarget(null);
    }
    if (googleSetupError?.type === service.type) {
      setGoogleSetupError(null);
    }
  };

  const renderConnectModal = (openService: GoogleConnectService) => {
    const config = configs.find((item) => item.integrationType === openService.type);
    const configured = Boolean(config);
    const serviceLiveData = liveData[openService.type];
    const active = isIntegrationActive(openService.type, configured, serviceLiveData);
    const setupError = hasSetupError(openService.type, configured, serviceLiveData);
    const googleCardSetupError =
      googleSetupError?.type === openService.type ? googleSetupError.message : null;

    return (
      <IntegrationModal
        title={openService.name}
        icon={<IntegrationServiceIconBadge type={openService.type} />}
        onClose={closeModal}>
        {active && configured ? (
          <IntegrationLiveDataPanel
            serviceType={openService.type}
            liveData={serviceLiveData}
            loading={loadingData[openService.type]}
            onRefresh={() => onFetchData(openService.type)}
            onDisconnect={() => onDisconnect(openService.type)}
          />
        ) : (
          <div className="subtle-divider-top integration-modal-body">
            <p className="muted-text">
              SiteCMD has read-only access to your Analytics and Search Console data and keeps it on
              your device. <ExtLink href="https://sitecmd.com/privacy">Privacy Policy</ExtLink>
            </p>
            <GoogleSignInButton
              onClick={() => void handleGoogleConnect(openService.type)}
              loading={googleConnecting}
            />
          </div>
        )}
        {googleCardSetupError ? (
          <div className="danger-callout-row integration-setup-error">
            <p className="text-body text-relaxed">{googleCardSetupError}</p>
            <Button
              onClick={() => handleGoogleConnect(openService.type)}
              disabled={googleConnecting}
              size="sm"
              className="btn--block">
              {googleConnecting ? "Authorizing..." : "Try setup again"}
            </Button>
          </div>
        ) : null}
        {setupError ? (
          <div className="danger-callout-row">
            <p className="text-body text-relaxed">Last check: {serviceLiveData?.error}</p>
          </div>
        ) : null}
      </IntegrationModal>
    );
  };

  return (
    <>
      {service ? renderConnectModal(service) : null}

      {googlePickerData ? (
        <GooglePicker
          data={googlePickerData}
          connectedTypes={connectedTypes}
          projectHost={url ? getHostname(url) : ""}
          targetType={googlePickerTarget}
          onPick={handlePickGoogleProperty}
          onClose={() => {
            setGooglePickerData(null);
            setGooglePickerTarget(null);
            setGoogleFlowId(null);
          }}
        />
      ) : null}
    </>
  );
}
