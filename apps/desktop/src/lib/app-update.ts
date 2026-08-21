import { safeListen } from "@/lib/tauri-events";
import {
  checkAppUpdate as checkAppUpdateCmd,
  downloadAndInstallAppUpdate,
  restartApp,
} from "@/lib/commands";

// The update-outcome unions are generated from the Rust enums (ts-rs), which
// serialize with a `kind` discriminator.
import type { UpdateCheckOutcome, UpdateInstallOutcome } from "@/generated/ipc-bindings";
export type { UpdateCheckOutcome, UpdateInstallOutcome };

// Mirrors Rust AppUpdateProgress + APP_UPDATE_PROGRESS_EVENT.
export interface AppUpdateProgress {
  downloaded: number;
  total: number | null;
}
const PROGRESS_EVENT = "app-update://progress";

/** Return typed update availability or failure without throwing. */
export async function checkAppUpdate(): Promise<UpdateCheckOutcome> {
  return checkAppUpdateCmd();
}

/** Install a Rust-verified update without relaunching the app. */
export async function installAppUpdate(
  onProgress?: (progress: AppUpdateProgress) => void,
): Promise<UpdateInstallOutcome> {
  let unlisten: (() => void) | undefined;
  if (onProgress) {
    unlisten = await safeListen<AppUpdateProgress>(PROGRESS_EVENT, (event) => {
      onProgress(event.payload);
    });
  }
  try {
    return await downloadAndInstallAppUpdate();
  } finally {
    unlisten?.();
  }
}

/** Relaunch after installing an update. */
export async function relaunchApp(): Promise<void> {
  await restartApp();
}

/** Download fraction, or null when the content length is unavailable. */
export function progressFraction(progress: AppUpdateProgress | null): number | null {
  if (!progress || !progress.total || progress.total <= 0) return null;
  return Math.min(1, progress.downloaded / progress.total);
}
