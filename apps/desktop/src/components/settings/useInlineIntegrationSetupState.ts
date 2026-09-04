import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import {
  completeGithubOauth,
  completeGoogleOauth,
  connectGithub,
  connectGoogle,
  saveGithubIntegration,
  saveGoogleIntegration,
  saveIntegration,
} from "@/lib/commands";
import { useToast } from "@/hooks/useToast";
import { useIntegrationsQuery } from "@/hooks/useIntegrationsQuery";
import { invalidateProjectMonitoringSignals } from "@/lib/project-summary-signals";
import { getHostname } from "@/lib/utils";
import type { IntegrationType } from "@/lib/types";
import { userFacingError } from "@/lib/user-facing-error";

import { getServiceName } from "./inline-integration-setup-model";
import {
  filterGooglePickerData,
  googleChoiceCount,
  googleIntegrationLabel,
  isGoogleIntegrationType,
  pickPreferredGoogleChoice,
  type GoogleIntegrationType,
  type GooglePickerData,
} from "./google-integration-selection";

type GitHubRepo = {
  full_name: string;
  description: string | null;
  private: boolean;
  default_branch: string;
  pushed_at: string | null;
};

interface UseInlineIntegrationSetupStateOptions {
  onConnected?: (type: string) => void;
  projectId: number;
  url?: string;
}

export function useInlineIntegrationSetupState({
  onConnected,
  projectId,
  url,
}: UseInlineIntegrationSetupStateOptions) {
  const { configs, loading: configsLoading, reload: loadConfigs } = useIntegrationsQuery(projectId);
  const [expandedService, setExpandedService] = useState<string | null>(null);
  const [apiKey, setApiKey] = useState("");
  const [siteId, setSiteId] = useState("");
  const [saving, setSaving] = useState(false);
  const toast = useToast();
  const queryClient = useQueryClient();

  const [googleConnecting, setGoogleConnecting] = useState(false);
  const [googlePickerData, setGooglePickerData] = useState<GooglePickerData | null>(null);
  const [googlePickerTarget, setGooglePickerTarget] = useState<GoogleIntegrationType | null>(null);
  const [googleFlowId, setGoogleFlowId] = useState<string | null>(null);
  const [googleError, setGoogleError] = useState<string | null>(null);

  const [ghConnecting, setGhConnecting] = useState(false);
  const [ghRepos, setGhRepos] = useState<GitHubRepo[] | null>(null);
  const [githubFlowId, setGithubFlowId] = useState<string | null>(null);
  const [ghDeviceCode, setGhDeviceCode] = useState<{
    userCode: string;
    verificationUri: string;
  } | null>(null);

  const getDefaultSiteId = useCallback(() => {
    if (!url) return "";
    return getHostname(url);
  }, [url]);

  const toggleApiService = useCallback(
    (type: string, hasSiteId: boolean) => {
      const isExpanded = expandedService === type;
      if (!isExpanded && hasSiteId) {
        setSiteId(getDefaultSiteId());
      }
      setExpandedService(isExpanded ? null : type);
      setApiKey("");
    },
    [expandedService, getDefaultSiteId],
  );

  const handleSave = async (type: string) => {
    setSaving(true);
    try {
      await saveIntegration({
        projectId,
        config: {
          integrationType: type as IntegrationType,
          apiKey: apiKey || null,
          siteId: siteId || null,
          extra: null,
          enabled: true,
        },
      });
      invalidateProjectMonitoringSignals(queryClient, projectId, url ?? null);
      setExpandedService(null);
      setApiKey("");
      setSiteId("");
      await loadConfigs();
      toast.success(`Connected`, `${getServiceName(type)} is now active.`);
      onConnected?.(type);
    } catch (e) {
      toast.error(`Failed to connect`, userFacingError(e, "Your change was not saved. Try again."));
    }
    setSaving(false);
  };

  const saveGoogleConnection = async (
    flowId: string,
    type: GoogleIntegrationType,
    selectedSiteId: string,
  ) => {
    await saveGoogleIntegration({
      projectId,
      flowId,
      integrationType: type,
      siteId: selectedSiteId,
    });
    invalidateProjectMonitoringSignals(queryClient, projectId, url ?? null);
    setGooglePickerData(null);
    setGooglePickerTarget(null);
    setGoogleFlowId(null);
    await loadConfigs();
    toast.success("Connected", `${googleIntegrationLabel(type)} is now active.`);
    onConnected?.(type);
  };

  const handleGoogleConnect = async (requestedType?: string) => {
    const target = isGoogleIntegrationType(requestedType) ? requestedType : null;
    setGoogleConnecting(true);
    setGooglePickerData(null);
    setGooglePickerTarget(target);
    setGoogleFlowId(null);
    setGoogleError(null);
    try {
      const started = await connectGoogle<{ flow_id: string }>({ projectId });
      setGoogleFlowId(started.flow_id);
      const data = await completeGoogleOauth<GooglePickerData>({
        projectId,
        flowId: started.flow_id,
      });

      // The backend may already have persisted this Google reconnect.
      const autoSaved = Array.isArray(data.auto_saved) ? data.auto_saved : [];
      if (autoSaved.length > 0) {
        await loadConfigs();
        autoSaved.forEach((savedType) => onConnected?.(savedType));
        if (!target || autoSaved.includes(target)) {
          setGooglePickerData(null);
          setGooglePickerTarget(null);
          setGoogleFlowId(null);
          return;
        }
      }

      const projectHost = url ? getHostname(url) : "";
      const preferredChoice = target ? pickPreferredGoogleChoice(data, target, projectHost) : null;
      if (target && preferredChoice) {
        await saveGoogleConnection(started.flow_id, target, preferredChoice);
        return;
      }
      if (target && googleChoiceCount(data, target) === 0) {
        toast.error(
          `${googleIntegrationLabel(target)} was not found`,
          "Make sure this Google account has access, then reconnect.",
        );
      }

      setGooglePickerData(filterGooglePickerData(data, target));
    } catch (e) {
      setGoogleError(userFacingError(e, "Your change was not saved. Try again."));
    } finally {
      setGoogleConnecting(false);
    }
  };

  const handlePickGoogleProperty = async (type: string, selectedSiteId: string) => {
    if (
      !googleFlowId ||
      !isGoogleIntegrationType(type) ||
      (googlePickerTarget !== null && type !== googlePickerTarget)
    ) {
      toast.error("Connection expired", "Reconnect Google and try again.");
      return;
    }
    try {
      await saveGoogleConnection(googleFlowId, type, selectedSiteId);
    } catch (e) {
      toast.error("Failed to save", userFacingError(e, "Your change was not saved. Try again."));
    }
  };

  const closeGooglePicker = () => {
    setGooglePickerData(null);
    setGooglePickerTarget(null);
    setGoogleFlowId(null);
  };

  const handleGitHubConnect = async () => {
    setGhConnecting(true);
    setGhDeviceCode(null);
    try {
      const started = await connectGithub<{
        flow_id: string;
        user_code: string;
        verification_uri: string;
      }>({ projectId });
      setGithubFlowId(started.flow_id);
      setGhDeviceCode({
        userCode: started.user_code,
        verificationUri: started.verification_uri,
      });
      toast.info("Enter this GitHub code", started.user_code);
      const data = await completeGithubOauth<{ repos: GitHubRepo[] }>({
        projectId,
        flowId: started.flow_id,
      });
      setGhRepos(data.repos);
      setGhDeviceCode(null);
    } catch (e) {
      setGhDeviceCode(null);
      toast.error("GitHub connection failed", userFacingError(e, "Try again in a moment."));
    }
    setGhConnecting(false);
  };

  const handlePickGitHubRepo = async (repo: string) => {
    if (!githubFlowId) {
      toast.error("Connection expired", "Reconnect GitHub and try again.");
      return;
    }
    try {
      await saveGithubIntegration({ projectId, flowId: githubFlowId, repo });
      invalidateProjectMonitoringSignals(queryClient, projectId, url ?? null);
      setGhRepos(null);
      setGithubFlowId(null);
      await loadConfigs();
      toast.success("Connected", `GitHub repository ${repo} is now active.`);
      onConnected?.("github");
    } catch (e) {
      toast.error("Failed to save", userFacingError(e, "Your change was not saved. Try again."));
    }
  };

  return {
    apiKey,
    configs,
    configsLoading,
    closeGooglePicker,
    expandedService,
    ghConnecting,
    ghDeviceCode,
    ghRepos,
    googleConnecting,
    googleError,
    googlePickerData,
    googlePickerTarget,
    handleGitHubConnect,
    handleGoogleConnect,
    handlePickGitHubRepo,
    handlePickGoogleProperty,
    handleSave,
    saving,
    setApiKey,
    setExpandedService,
    setGhRepos,
    setSiteId,
    siteId,
    toggleApiService,
  };
}
