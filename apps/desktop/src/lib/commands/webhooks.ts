import { command } from "./invoke";
import type { WebhookConfig } from "@/generated/ipc-bindings";

export function getWebhookConfigs(args: { projectId: number }): Promise<WebhookConfig[]> {
  return command<WebhookConfig[]>("get_webhook_configs", args);
}

export function saveWebhookConfig(args: {
  projectId: number;
  url: string;
  events: string;
  secret?: string | null;
  enabled: boolean;
}): Promise<number> {
  return command<number>("save_webhook_config", args);
}

export function deleteWebhookConfig(args: { id: number }): Promise<void> {
  return command<void>("delete_webhook_config", args);
}

export function testWebhook(args: { id: number }): Promise<string> {
  return command<string>("test_webhook", args);
}
