import { command } from "./invoke";
import type { IntegrationConfig, IntegrationData } from "@/generated/ipc-bindings";

export function saveIntegration(args: {
  projectId: number;
  config: IntegrationConfig;
}): Promise<void> {
  return command<void>("save_integration", args);
}

export function getIntegrations(args: { projectId: number }): Promise<IntegrationConfig[]> {
  return command<IntegrationConfig[]>("get_integrations", args);
}

export function deleteIntegration(args: {
  projectId: number;
  integrationType: string;
}): Promise<void> {
  return command<void>("delete_integration", args);
}

export function fetchIntegrationData(args: {
  projectId: number;
  integrationType: string;
  urlFilter?: string | null;
}): Promise<IntegrationData> {
  return command<IntegrationData>("fetch_integration_data", args);
}

export function fetchGithubData<T = unknown>(args: { projectId: number }): Promise<T> {
  return command<T>("fetch_github_data", args);
}

export function fetchAnalytics<T = unknown>(args: {
  projectId: number;
  period: string;
  siteUrl?: string | null;
}): Promise<T> {
  return command<T>("fetch_analytics", args);
}

export function invalidateAnalyticsCache(args: { projectId: number }): Promise<void> {
  return command<void>("invalidate_analytics_cache", args);
}

export function dismissIntegrationHint(args: {
  projectId: number;
  checkId: string;
  integrationType: string;
}): Promise<void> {
  return command<void>("dismiss_integration_hint", args);
}

export function connectGoogle<T = unknown>(args: { projectId: number }): Promise<T> {
  return command<T>("connect_google", args);
}

export function completeGoogleOauth<T = unknown>(args: {
  projectId: number;
  flowId: string;
}): Promise<T> {
  return command<T>("complete_google_oauth", args);
}

export function saveGoogleIntegration(args: {
  projectId: number;
  flowId: string;
  integrationType: string;
  siteId: string;
}): Promise<string> {
  return command<string>("save_google_integration", args);
}

export function connectGithub<T = unknown>(args: { projectId: number }): Promise<T> {
  return command<T>("connect_github", args);
}

export function completeGithubOauth<T = unknown>(args: {
  projectId: number;
  flowId: string;
}): Promise<T> {
  return command<T>("complete_github_oauth", args);
}

export function saveGithubIntegration(args: {
  projectId: number;
  flowId: string;
  repo: string;
}): Promise<string> {
  return command<string>("save_github_integration", args);
}
