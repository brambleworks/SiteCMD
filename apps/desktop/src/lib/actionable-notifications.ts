import { sendActionableDesktopNotification as sendActionableDesktopNotificationCmd } from "@/lib/commands";
import type { ActionableDesktopNotificationRequest as WireNotificationRequest } from "@/generated/ipc-bindings";
import type { AppTarget } from "@/lib/app-targets";

export interface ActionableDesktopNotificationAction {
  id: string;
  label: string;
  target?: AppTarget | null;
  filePath?: string | null;
}

export interface ActionableDesktopNotificationRequest {
  id?: string | null;
  title: string;
  body: string;
  clickTarget?: AppTarget | null;
  actions?: ActionableDesktopNotificationAction[];
}

export interface ActionableDesktopNotificationEvent {
  sourceId?: string | null;
  actionId: string;
  target?: AppTarget | null;
  filePath?: string | null;
}

export async function sendActionableDesktopNotification(
  request: ActionableDesktopNotificationRequest,
): Promise<boolean> {
  try {
    // AppTarget and DesktopNotificationTarget share the serialized wire shape.
    await sendActionableDesktopNotificationCmd({
      request: request as unknown as WireNotificationRequest,
    });
    return true;
  } catch {
    return false;
  }
}
