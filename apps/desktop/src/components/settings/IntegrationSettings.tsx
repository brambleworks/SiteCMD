import { useState, useEffect, useCallback, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { deleteIntegration, fetchIntegrationData } from "@/lib/commands";
import { useToast } from "@/hooks/useToast";
import { invalidateProjectMonitoringSignals } from "@/lib/project-summary-signals";
import { queryKeys } from "@/lib/query/query-keys";
import { getHostname } from "@/lib/utils";
import { useIntegrationsQuery } from "@/hooks/useIntegrationsQuery";
import {
  SERVICES,
  GOOGLE_SERVICES,
  GITHUB_SERVICE,
  JIRA_SERVICE,
  SERVICE_META,
  SERVICE_CATEGORIES,
  SERVICE_CATEGORY_INFO,
  integrationDisplayName,
} from "./integration-services";
import type { IntegrationConfig, IntegrationData } from "./integration-services";
import { isIntegrationActive } from "./integration-connection-status";
import { IntegrationServiceIconBadge } from "./IntegrationServicePanels";
import { IntegrationRow } from "./IntegrationRow";
import { AgentToolCards } from "./AgentToolCards";
import { ApiKeyIntegrationConnect } from "./ApiKeyIntegrationConnect";
import { JiraIntegrationConnect } from "./JiraIntegrationConnect";
import { GoogleIntegrationConnect } from "./GoogleIntegrationConnect";

interface IntegrationSettingsProps {
  projectId: number;
  projectName: string;
  url?: string;
  focusIntegration?: string | null;
  configs?: IntegrationConfig[];
  onReloadConfigs?: () => Promise<IntegrationConfig[]>;
}

const ALL_INTEGRATION_SERVICES = [
  ...SERVICES.map((service) => ({ ...service, flow: "apikey" as const })),
  ...GOOGLE_SERVICES.map((service) => ({ ...service, flow: "google" as const })),
  { ...GITHUB_SERVICE, flow: "apikey" as const },
  { ...JIRA_SERVICE, flow: "jira" as const },
];
type IntegrationServiceDef = (typeof ALL_INTEGRATION_SERVICES)[number];

function getInitialSiteId(type: string, hasSiteIdLabel: boolean, url?: string) {
  if (!hasSiteIdLabel) return "";
  if (type === "github") return "";
  return url ? getHostname(url) : "";
}

interface ModalSeed {
  nonce: number;
  initialSiteId: string;
}

function initialModalSiteId(
  service: IntegrationServiceDef,
  configs: IntegrationConfig[],
  url?: string,
) {
  if (!("siteIdLabel" in service)) return "";
  const config = configs.find((item) => item.integrationType === service.type);
  return config?.siteId ?? getInitialSiteId(service.type, Boolean(service.siteIdLabel), url);
}

export function IntegrationSettings({
  projectId,
  url,
  focusIntegration,
  configs: suppliedConfigs,
  onReloadConfigs,
}: IntegrationSettingsProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [modalService, setModalService] = useState<string | null>(null);
  const [modalSeed, setModalSeed] = useState<ModalSeed>({ nonce: 0, initialSiteId: "" });
  const [liveData, setLiveData] = useState<Record<string, IntegrationData>>({});
  const [loadingData, setLoadingData] = useState<Record<string, boolean>>({});
  const integrationsQuery = useIntegrationsQuery(projectId);
  const configs = suppliedConfigs ?? integrationsQuery.configs;
  const reloadIntegrations = integrationsQuery.reload;

  const fetchData = useCallback(
    async (type: string, force = false) => {
      setLoadingData((prev) => ({ ...prev, [type]: true }));
      try {
        const hostname = url ? getHostname(url) : "";
        const data = await queryClient.fetchQuery<IntegrationData>({
          queryKey: queryKeys.integrations.data(projectId, type, hostname),
          queryFn: () =>
            fetchIntegrationData({
              projectId,
              integrationType: type,
              urlFilter: hostname,
            }) as Promise<IntegrationData>,
          ...(force ? { staleTime: 0 } : {}),
        });
        setLiveData((prev) => ({ ...prev, [type]: data }));
      } catch (error) {
        setLiveData((prev) => ({
          ...prev,
          [type]: {
            integrationType: type,
            data: {},
            fetchedAt: new Date().toISOString(),
            error: String(error),
          },
        }));
      }
      setLoadingData((prev) => ({ ...prev, [type]: false }));
    },
    [projectId, queryClient, url],
  );

  useEffect(() => {
    setLiveData({});
    for (const config of configs.filter((item) => item.enabled)) {
      void fetchData(config.integrationType);
    }
  }, [configs, fetchData]);

  const loadConfigs = useCallback(async () => {
    await (onReloadConfigs?.() ?? reloadIntegrations());
  }, [onReloadConfigs, reloadIntegrations]);

  const handleDelete = async (type: string) => {
    try {
      await deleteIntegration({ projectId, integrationType: type });
      invalidateProjectMonitoringSignals(queryClient, projectId, url ?? null);
      setLiveData((prev) => {
        const next = { ...prev };
        delete next[type];
        return next;
      });
      setModalService(null);
      await loadConfigs();
      toast.info(`${integrationDisplayName(type)} disconnected`, "Integration removed.");
    } catch (e) {
      toast.error(`Failed to disconnect ${integrationDisplayName(type)}`, String(e));
    }
  };

  const connectedTypes = new Set(configs.map((c) => c.integrationType));

  const isActive = (type: string) =>
    isIntegrationActive(type, connectedTypes.has(type), liveData[type]);

  const activeServices = ALL_INTEGRATION_SERVICES.filter((service) => isActive(service.type));

  const categoryGroups = SERVICE_CATEGORIES.filter((category) => category !== "All")
    .map((category) => ({
      category,
      description: SERVICE_CATEGORY_INFO[category],
      services: ALL_INTEGRATION_SERVICES.filter(
        (service) => SERVICE_META[service.type]?.category === category && !isActive(service.type),
      ),
    }))
    .filter((group) => group.services.length > 0);

  const openServiceModal = (service: IntegrationServiceDef) => {
    setModalSeed((seed) => ({
      nonce: seed.nonce + 1,
      initialSiteId: initialModalSiteId(service, configs, url),
    }));
    setModalService(service.type);
  };

  const closeModal = () => setModalService(null);

  useEffect(() => {
    if (!focusIntegration) return;
    const service = ALL_INTEGRATION_SERVICES.find((item) => item.type === focusIntegration);
    if (!service) return;
    setModalSeed((seed) => ({
      nonce: seed.nonce + 1,
      initialSiteId: initialModalSiteId(service, configs, url),
    }));
    setModalService(focusIntegration);
  }, [configs, focusIntegration, url]);

  const renderRow = (service: IntegrationServiceDef) => (
    <IntegrationRow
      key={service.type}
      dataIntegration={service.type}
      icon={<IntegrationServiceIconBadge type={service.type} />}
      name={service.name}
      connected={isActive(service.type)}
      actionLabel={connectedTypes.has(service.type) ? "Manage" : "Set up"}
      onOpen={() => openServiceModal(service)}
    />
  );

  const modalServiceDef = modalService
    ? (ALL_INTEGRATION_SERVICES.find((service) => service.type === modalService) ?? null)
    : null;

  return (
    <div className="stack-hero">
      {activeServices.length > 0 ? (
        <IntegrationSection title="Active">{activeServices.map(renderRow)}</IntegrationSection>
      ) : null}

      <AgentToolCards />

      {categoryGroups.map((group) => (
        <IntegrationSection
          key={group.category}
          title={group.category}
          description={group.description}>
          {group.services.map(renderRow)}
        </IntegrationSection>
      ))}

      {modalServiceDef && modalServiceDef.flow === "apikey" ? (
        <ApiKeyIntegrationConnect
          key={`${modalServiceDef.type}:${modalSeed.nonce}`}
          service={modalServiceDef}
          config={configs.find((item) => item.integrationType === modalServiceDef.type)}
          initialSiteId={modalSeed.initialSiteId}
          liveData={liveData[modalServiceDef.type]}
          loading={loadingData[modalServiceDef.type]}
          projectId={projectId}
          url={url}
          onRefresh={() => fetchData(modalServiceDef.type, true)}
          onDisconnect={() => handleDelete(modalServiceDef.type)}
          onClose={closeModal}
          onReloadConfigs={loadConfigs}
        />
      ) : null}

      <JiraIntegrationConnect
        open={modalService === JIRA_SERVICE.type}
        config={configs.find((item) => item.integrationType === JIRA_SERVICE.type)}
        projectId={projectId}
        url={url}
        onClose={closeModal}
        onDisconnect={() => handleDelete(JIRA_SERVICE.type)}
        onReloadConfigs={loadConfigs}
      />

      <GoogleIntegrationConnect
        service={modalServiceDef?.flow === "google" ? modalServiceDef : null}
        configs={configs}
        liveData={liveData}
        loadingData={loadingData}
        projectId={projectId}
        url={url}
        onFetchData={(type) => fetchData(type, true)}
        onDisconnect={handleDelete}
        onReloadConfigs={loadConfigs}
        onModalServiceChange={setModalService}
      />

      <p className="integration-credentials-note text-body-muted text-relaxed">
        All credentials are stored locally on your machine and only used to communicate directly
        with each service.
      </p>
    </div>
  );
}

function IntegrationSection({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="stack-base">
      <div className="stack-tight">
        <p className="row-title-md">{title}</p>
        {description ? <p className="text-13-muted text-relaxed">{description}</p> : null}
      </div>
      <div className="integration-section-list">{children}</div>
    </section>
  );
}
