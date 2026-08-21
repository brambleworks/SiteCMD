import { writeText } from "@tauri-apps/plugin-clipboard-manager";

/** Copy text to system clipboard via Tauri plugin. */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await writeText(text);
    return true;
  } catch {
    return false;
  }
}
