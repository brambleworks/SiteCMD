import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { saveIntegration } from "@/lib/commands";
import { useToast } from "@/hooks/useToast";
import { invalidateProjectMonitoringSignals } from "@/lib/project-summary-signals";
import { JIRA_SERVICE } from "./integration-services";
import type { IntegrationConfig } from "./integration-services";
import {
  IntegrationServiceIconBadge,
  JiraIntegrationForm,
  type JiraFormValue,
} from "./IntegrationServicePanels";
import { IntegrationModal } from "./IntegrationModal";

interface JiraIntegrationConnectProps {
  open: boolean;
  config: IntegrationConfig | undefined;
  projectId: number;
  url?: string;
  onClose: () => void;
  onDisconnect: () => void;
  onReloadConfigs: () => Promise<void>;
}

/** Keep Jira form values mounted across modal open and close. */
export function JiraIntegrationConnect({
  open,
  config,
  projectId,
  url,
  onClose,
  onDisconnect,
  onReloadConfigs,
}: JiraIntegrationConnectProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [instanceUrl, setInstanceUrl] = useState("");
  const [email, setEmail] = useState("");
  const [apiToken, setApiToken] = useState("");
  const [projectKey, setProjectKey] = useState("");
  const [issueType, setIssueType] = useState("Bug");
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!config) return;
    const extra = config.extra as Record<string, string> | null;
    if (!extra) return;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- populates the editable form from the loaded integration config; re-syncs when config changes
    setInstanceUrl(extra.instance_url ?? "");
    setEmail(extra.email ?? "");
    setProjectKey(extra.project_key ?? "");
    setIssueType(extra.issue_type ?? "Bug");
  }, [config]);

  const form: JiraFormValue = {
    instanceUrl,
    email,
    apiToken,
    projectKey,
    issueType,
  };

  const updateForm = (next: Partial<JiraFormValue>) => {
    if (next.instanceUrl !== undefined) setInstanceUrl(next.instanceUrl);
    if (next.email !== undefined) setEmail(next.email);
    if (next.apiToken !== undefined) setApiToken(next.apiToken);
    if (next.projectKey !== undefined) setProjectKey(next.projectKey);
    if (next.issueType !== undefined) setIssueType(next.issueType);
  };

  const handleClose = () => {
    setShowKey(false);
    onClose();
  };

  const handleSave = async () => {
    if (!instanceUrl || !email || !apiToken || !projectKey) {
      toast.error("Missing fields", "All fields except issue type are required");
      return;
    }
    setSaving(true);
    try {
      await saveIntegration({
        projectId,
        config: {
          integrationType: "jira",
          apiKey: apiToken,
          siteId: null,
          extra: {
            instance_url: instanceUrl,
            email,
            project_key: projectKey,
            issue_type: issueType,
          },
          enabled: true,
        },
      });
      invalidateProjectMonitoringSignals(queryClient, projectId, url ?? null);
      toast.success("Jira connected", `Project: ${projectKey}`);
      setApiToken("");
      handleClose();
      await onReloadConfigs();
    } catch (e) {
      toast.error("Failed to save Jira config", String(e));
    }
    setSaving(false);
  };

  if (!open) return null;

  const configured = Boolean(config);

  return (
    <IntegrationModal
      title={JIRA_SERVICE.name}
      icon={<IntegrationServiceIconBadge type={JIRA_SERVICE.type} />}
      onClose={handleClose}>
      <JiraIntegrationForm
        form={form}
        showKey={showKey}
        saving={saving}
        submitLabel={configured ? "Save changes" : "Connect"}
        savingLabel={configured ? "Saving…" : "Connecting…"}
        onChange={updateForm}
        onToggleShowKey={() => setShowKey(!showKey)}
        onSave={handleSave}
        onCancel={handleClose}
        onDisconnect={configured ? onDisconnect : undefined}
      />
    </IntegrationModal>
  );
}
