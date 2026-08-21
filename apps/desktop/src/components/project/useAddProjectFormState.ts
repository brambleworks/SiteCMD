import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { addEnvironmentUrl, addProject, addProjectByUrl, detectProjectUrls } from "@/lib/commands";
import {
  getProjectUrlIdentityKey,
  inferProjectEnvironmentFromUrl,
  isLoopbackProjectUrl,
  normalizeProjectUrlInput,
  resolveProjectEnvironmentForUrl,
  type ProjectEnvironment,
} from "@/lib/project-environments";
import { recordWorkflowHealthEvent } from "@/lib/observability";

import { buildInitialUrls, type UrlRow } from "./add-project-form-model";

interface UseAddProjectFormStateOptions {
  onCreated: (projectId: number) => void;
}

function normalizeUrlRows(rows: UrlRow[]): UrlRow[] {
  const seen = new Set<string>();
  const normalized: UrlRow[] = [];

  for (const row of rows) {
    const url = row.url.trim();
    if (!url) continue;
    const identity = getProjectUrlIdentityKey(url);
    if (!identity || seen.has(identity)) continue;
    seen.add(identity);
    normalized.push({
      url,
      environment: resolveProjectEnvironmentForUrl(url, row.environment),
    });
  }

  return normalized;
}

export function useAddProjectFormState({ onCreated }: UseAddProjectFormStateOptions) {
  const [name, setName] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [framework, setFramework] = useState<string | null>(null);
  const [urls, setUrls] = useState<UrlRow[]>(() => buildInitialUrls());
  const [primaryEnvironmentTouched, setPrimaryEnvironmentTouched] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [saving, setSaving] = useState(false);
  const [folderError, setFolderError] = useState<string | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [folderNotice, setFolderNotice] = useState<string | null>(null);

  const handleSelectFolder = async () => {
    setFolderError(null);
    setFolderNotice(null);
    try {
      const selected = await open({ directory: true, title: "Select project folder" });
      if (!selected) return;
      const path = typeof selected === "string" ? selected : selected;
      setFolder(path);
      setScanning(true);

      const info = await detectProjectUrls({ path });
      if (!name) setName(info.name);
      if (info.framework) setFramework(info.framework);

      const manual = urls.filter((u) => u.url.trim());
      // Do not turn detected dev-server guesses into project environments.
      const detected = info.urls
        .filter((u) => !isLoopbackProjectUrl(u.url))
        .map((u) => ({
          url: u.url,
          environment: u.environment as ProjectEnvironment,
        }));
      const merged = normalizeUrlRows([...manual, ...detected]);
      if (merged.length === 0) {
        setFolderNotice(
          "No environment URL was detected. You can create this project with the folder alone and run Code Scan, or add a URL to scan the live site too.",
        );
      }
      setUrls(merged.length > 0 ? merged : buildInitialUrls());
      setPrimaryEnvironmentTouched(false);
      setScanning(false);
    } catch (error) {
      const message =
        typeof error === "string"
          ? error
          : (error as Error)?.message ||
            "We couldn't inspect that folder. You can still continue by entering your site URLs manually.";
      setFolderError(message);
      setScanning(false);
    }
  };

  const handleRemoveFolder = () => {
    setFolder(null);
    setFramework(null);
    setFolderError(null);
    setFolderNotice(null);
  };

  const addUrlRow = () => setUrls([...urls, { url: "", environment: "staging" }]);

  const updateUrl = (index: number, field: "url" | "environment", value: string) => {
    setUrls((current) =>
      current.map((u, i) => {
        if (i !== index) return u;
        const next = { ...u, [field]: value };
        if (index === 0 && field === "url" && !primaryEnvironmentTouched) {
          next.environment = inferProjectEnvironmentFromUrl(value);
        }
        return next;
      }),
    );
    if (index === 0 && field === "environment") {
      setPrimaryEnvironmentTouched(true);
    }
  };

  const removeUrl = (index: number) => {
    if (urls.length <= 1) return;
    setUrls(urls.filter((_, i) => i !== index));
  };

  const handleSubmit = async () => {
    if (!name.trim()) return;
    // A project requires either a site or codebase; code-only projects need no URL.
    const effectiveUrls = normalizeUrlRows(urls);
    if (effectiveUrls.length === 0 && !folder) return;

    setSaving(true);
    setSubmitError(null);
    // Set as soon as the project row exists, so the catch below can tell
    // "nothing was created" from "created, then a later step failed".
    let createdProjectId: number | null = null;
    recordWorkflowHealthEvent("add_site", "started", {
      mode: folder ? "folder" : "url",
      primaryEnvironment: effectiveUrls[0]?.environment ?? null,
      environmentCount: effectiveUrls.length,
      hasFolder: Boolean(folder),
    });
    try {
      if (folder) {
        const projectId = await addProject({
          name: name.trim(),
          path: folder,
          framework,
          urls: effectiveUrls.map((entry) => ({
            url: normalizeProjectUrlInput(entry.url),
            environment: entry.environment,
            source: "manual",
          })),
        });
        recordWorkflowHealthEvent("add_site", "succeeded", {
          mode: "folder",
          primaryEnvironment: effectiveUrls[0]?.environment ?? null,
          environmentCount: effectiveUrls.length,
        });
        onCreated(projectId);
      } else {
        const firstUrl = normalizeProjectUrlInput(effectiveUrls[0].url);
        const projectId = await addProjectByUrl({
          name: name.trim(),
          url: firstUrl,
        });
        createdProjectId = projectId;
        for (const u of effectiveUrls.slice(1)) {
          const normalized = normalizeProjectUrlInput(u.url);
          await addEnvironmentUrl({
            projectId,
            url: normalized,
            label: `${name.trim()} (${u.environment.charAt(0).toUpperCase() + u.environment.slice(1)})`,
            environment: u.environment,
          });
        }
        recordWorkflowHealthEvent("add_site", "succeeded", {
          mode: "url",
          primaryEnvironment: effectiveUrls[0]?.environment ?? null,
          environmentCount: effectiveUrls.length,
        });
        onCreated(projectId);
      }
    } catch (err) {
      const msg = typeof err === "string" ? err : (err as Error)?.message || "";
      recordWorkflowHealthEvent("add_site", "failed", {
        mode: folder ? "folder" : "url",
        errorType: "submit",
        hasFolder: Boolean(folder),
      });
      if (folder) {
        setFolderError(
          msg ||
            "We couldn't finish inspecting that folder. You can still review the linked folder and URLs, then try again.",
        );
      } else {
        // Surface URL-mode failures instead of leaving the dialog silently open.
        setSubmitError(
          msg || "We couldn't finish creating that project. Check the URLs and try again.",
        );
      }
      // Return partially created projects so retries cannot create duplicates.
      if (createdProjectId !== null) {
        onCreated(createdProjectId);
      }
    }
    setSaving(false);
  };

  const hasValidUrl = urls.some((u) => u.url.trim());

  return {
    addUrlRow,
    folder,
    folderError,
    folderNotice,
    framework,
    handleRemoveFolder,
    handleSelectFolder,
    handleSubmit,
    hasValidUrl,
    submitError,
    name,
    removeUrl,
    saving,
    scanning,
    setName,
    updateUrl,
    urls,
  };
}
