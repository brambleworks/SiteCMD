import { command } from "./invoke";
import type {
  UpdateCheckOutcome,
  UpdateInstallOutcome,
  UpdateReport,
} from "@/generated/ipc-bindings";

export function detectUpdates(args: {
  projectId: number;
  projectPath?: string | null;
}): Promise<UpdateReport> {
  return command<UpdateReport>("detect_updates", args);
}

export function checkAppUpdate(): Promise<UpdateCheckOutcome> {
  return command<UpdateCheckOutcome>("check_app_update");
}

export function downloadAndInstallAppUpdate(): Promise<UpdateInstallOutcome> {
  return command<UpdateInstallOutcome>("download_and_install_app_update");
}

export function restartApp(): Promise<void> {
  return command<void>("restart_app");
}
