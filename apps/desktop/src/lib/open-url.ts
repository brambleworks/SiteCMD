import { isTauri } from "@tauri-apps/api/core";
import { openExternalUrl } from "@/lib/commands";

function parseSafeHttpUrl(value: string): string | null {
  let parsed: URL;
  try {
    parsed = new URL(value.trim());
  } catch {
    return null;
  }
  if (parsed.protocol !== "https:" && parsed.protocol !== "http:") return null;
  if (parsed.username || parsed.password) return null;
  return parsed.toString();
}

export async function openUrl(url: string): Promise<void> {
  const safeUrl = parseSafeHttpUrl(url);
  if (!safeUrl) {
    console.warn("openUrl: blocked unsafe external URL");
    return;
  }
  if (isTauri()) {
    await openExternalUrl({ url: safeUrl });
    return;
  }
  if (import.meta.env.DEV || import.meta.env.MODE === "test") {
    window.open(safeUrl, "_blank", "noopener,noreferrer");
    return;
  }
  throw new Error("External links require the SiteCMD desktop security boundary.");
}
