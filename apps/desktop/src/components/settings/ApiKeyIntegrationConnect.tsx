import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { saveIntegration } from "@/lib/commands";
import { useToast } from "@/hooks/useToast";
import { invalidateProjectMonitoringSignals } from "@/lib/project-summary-signals";
import type { IntegrationType } from "@/lib/types";
import { integrationDisplayName } from "./integration-services";
import type { IntegrationConfig, IntegrationData } from "./integration-services";
import { hasSetupError, isIntegrationActive } from "./integration-connection-status";
import {
  ApiKeyIntegrationSetup,
  IntegrationLiveDataPanel,
  IntegrationServiceIconBadge,
} from "./IntegrationServicePanels";
import { IntegrationModal } from "./IntegrationModal";

interface ApiKeyConnectService {
  type: string;
  name: string;
  keyLabel: string;
  setupSteps: readonly string[];
  setupUrl?: string;
  setupUrlLabel?: string;
  siteIdLabel?: string | null;
  siteIdPlaceholder?: string | null;
  siteIdHelp?: string | null;
}

interface ApiKeyIntegrationConnectProps {
  service: ApiKeyConnectService;
  config: IntegrationConfig | undefined;
  initialSiteId: string;
  liveData: IntegrationData | undefined;
  loading: boolean | undefined;
  projectId: number;
  url?: string;
  onRefresh: () => void;
  onDisconnect: () => void;
  onClose: () => void;
  onReloadConfigs: () => Promise<void>;
}

/** API-key connection form remounted for each modal session. */
export function ApiKeyIntegrationConnect({
  service,
  config,
  initialSiteId,
  liveData,
  loading,
  projectId,
  url,
  onRefresh,
  onDisconnect,
  onClose,
  onReloadConfigs,
}: ApiKeyIntegrationConnectProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [apiKey, setApiKey] = useState("");
  const [siteId, setSiteId] = useState(initialSiteId);
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);

  const configured = Boolean(config);
  const active = isIntegrationActive(service.type, configured, liveData);
  const setupError = hasSetupError(service.type, configured, liveData);

  const handleSave = async () => {
    setSaving(true);
    try {
      await saveIntegration({
        projectId,
        config: {
          integrationType: service.type as IntegrationType,
          apiKey: apiKey || null,
          siteId: siteId || null,
          extra: null,
          enabled: true,
        },
      });
      invalidateProjectMonitoringSignals(queryClient, projectId, url ?? null);
      onClose();
      await onReloadConfigs();
      // A credential replacement can serialize to the same redacted config,
      // so a config effect alone is not a reliable live-data refresh trigger.
      onRefresh();
      toast.success(`${integrationDisplayName(service.type)} connected`, "Integration saved.");
    } catch (e) {
      toast.error(`Failed to save ${integrationDisplayName(service.type)}`, String(e));
    }
    setSaving(false);
  };

  return (
    <IntegrationModal
      title={service.name}
      icon={<IntegrationServiceIconBadge type={service.type} />}
      onClose={onClose}>
      {active && configured ? (
        <IntegrationLiveDataPanel
          serviceType={service.type}
          liveData={liveData}
          loading={loading}
          onRefresh={onRefresh}
          onDisconnect={onDisconnect}
        />
      ) : (
        <ApiKeyIntegrationSetup
          service={service}
          apiKey={apiKey}
          siteId={siteId}
          showKey={showKey}
          saving={saving}
          submitLabel={configured ? "Save credentials" : "Connect"}
          savingLabel={configured ? "Saving..." : "Connecting..."}
          onApiKeyChange={setApiKey}
          onSiteIdChange={setSiteId}
          onToggleShowKey={() => setShowKey(!showKey)}
          onSave={handleSave}
          onCancel={onClose}
        />
      )}
      {setupError ? (
        <div className="danger-callout-row">
          <p className="text-body text-relaxed">Last check: {liveData?.error}</p>
        </div>
      ) : null}
    </IntegrationModal>
  );
}
