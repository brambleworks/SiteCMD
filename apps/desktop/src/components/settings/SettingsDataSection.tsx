import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { save, open as openDialog } from "@tauri-apps/plugin-dialog";
import { Download, FolderOpen, Trash2, Upload, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  addEnvironmentUrl,
  clearScanHistory,
  deleteEnvironment,
  deleteProject,
  exportDatabase,
  getDbPath,
  getDbSize,
  importDatabase,
  renameProject,
  updateProjectPath,
} from "@/lib/commands";
import { formatBytes } from "@/lib/tokens";
import {
  inferProjectEnvironmentFromUrl,
  normalizeProjectUrlInput,
  type ProjectEnvironment,
} from "@/lib/project-environments";
import { useToast } from "@/hooks/useToast";
import type { EnvironmentRecord } from "@/hooks/useProject";
import { queryKeys } from "@/lib/query/query-keys";
import { InlineSkeleton } from "@/components/ui/skeleton";

interface DataSectionProps {
  view?: "site-setup" | "data";
  framework?: string | null;
  projectId?: number;
  projectName?: string;
  projectPath?: string | null;
  projectEnvironments?: EnvironmentRecord[];
  onProjectChanged?: () => void | Promise<unknown>;
}

function formatEnvironmentLabel(environment: string): string {
  if (environment === "local") return "Local";
  if (environment === "development") return "Development";
  if (environment === "staging") return "Staging";
  return "Production";
}

export function DataSection({
  view = "site-setup",
  framework,
  projectId,
  projectName,
  projectPath,
  projectEnvironments = [],
  onProjectChanged,
}: DataSectionProps) {
  const queryClient = useQueryClient();
  const dbInfoQuery = useQuery({
    queryKey: queryKeys.settings.databaseInfo(),
    queryFn: async () => {
      const path = await getDbPath();
      try {
        return { path, size: formatBytes(await getDbSize()) };
      } catch {
        return { path, size: "unknown" };
      }
    },
    enabled: view === "data",
    // Revalidate runtime database size while retaining the cached value.
    staleTime: 0,
  });
  const [confirmClear, setConfirmClear] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [editName, setEditName] = useState(projectName || "");
  const [renaming, setRenaming] = useState(false);
  const [changingProjectPath, setChangingProjectPath] = useState(false);
  const [unlinkingProjectPath, setUnlinkingProjectPath] = useState(false);
  const [newEnvironmentUrl, setNewEnvironmentUrl] = useState("");
  const [newEnvironmentType, setNewEnvironmentType] = useState<ProjectEnvironment>("staging");
  const [addingEnvironment, setAddingEnvironment] = useState(false);
  const [deletingEnvironmentId, setDeletingEnvironmentId] = useState<number | null>(null);
  const [environmentTypeTouched, setEnvironmentTypeTouched] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const { success, error: showError } = useToast();

  // Reseed the editable name when the projectName prop changes, adjusting state
  // during render instead of via an effect.
  const [renderedProjectName, setRenderedProjectName] = useState(projectName);
  if (renderedProjectName !== projectName) {
    setRenderedProjectName(projectName);
    setEditName(projectName || "");
  }

  const handleClear = async () => {
    setClearing(true);
    try {
      const deleted = await clearScanHistory();
      setConfirmClear(false);
      await queryClient.invalidateQueries();
      success(
        "Scan history cleared",
        `Removed ${deleted} scan${deleted !== 1 ? "s" : ""} and all associated issues.`,
      );
    } catch (e) {
      showError("Failed to clear history", String(e));
    } finally {
      setClearing(false);
    }
  };

  const handleExport = async () => {
    setExporting(true);
    try {
      const filePath = await save({
        title: "Export SiteCMD Database",
        defaultPath: `sitecmd-backup-${new Date().toISOString().slice(0, 10)}.db`,
        filters: [{ name: "SQLite Database", extensions: ["db"] }],
      });
      if (!filePath) {
        setExporting(false);
        return;
      }
      const sizeStr = await exportDatabase({ destPath: filePath });
      success("Backup exported", `Database saved (${sizeStr})`);
    } catch (e) {
      showError("Export failed", String(e));
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    const filePath = await openDialog({
      title: "Import SiteCMD Backup",
      filters: [{ name: "SQLite Database", extensions: ["db"] }],
      multiple: false,
      directory: false,
    });
    if (!filePath) return;
    setImporting(true);
    try {
      const sizeStr = await importDatabase({ srcPath: filePath });
      await queryClient.invalidateQueries();
      success(
        "Backup restored",
        `Database imported (${sizeStr}). SiteCMD is now using the restored data.`,
      );
    } catch (e) {
      showError("Import failed", String(e));
    } finally {
      setImporting(false);
    }
  };

  const handleRename = async () => {
    if (!projectId || !editName.trim() || editName.trim() === projectName) return;
    setRenaming(true);
    try {
      await renameProject({ projectId, name: editName.trim() });
      success("Project renamed", `Now called "${editName.trim()}"`);
      await Promise.resolve(onProjectChanged?.());
    } catch (e) {
      showError("Rename failed", String(e));
    } finally {
      setRenaming(false);
    }
  };

  const handleChangeProjectPath = async () => {
    if (!projectId) return;
    setChangingProjectPath(true);
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "Select project folder",
      });
      if (!selected) return;
      const path = typeof selected === "string" ? selected : String(selected);
      await updateProjectPath({ projectId, path });
      success("Project folder updated", "Code Scan and dependency checks will use this folder.");
      await Promise.resolve(onProjectChanged?.());
    } catch (e) {
      showError("Could not update project folder", String(e));
    } finally {
      setChangingProjectPath(false);
    }
  };

  const handleUnlinkProjectPath = async () => {
    if (!projectId) return;
    setUnlinkingProjectPath(true);
    try {
      await updateProjectPath({ projectId, path: "" });
      success("Project folder unlinked", "Code Scan is disabled until another folder is linked.");
      await Promise.resolve(onProjectChanged?.());
    } catch (e) {
      showError("Could not unlink project folder", String(e));
    } finally {
      setUnlinkingProjectPath(false);
    }
  };

  const handleNewEnvironmentUrlChange = (value: string) => {
    setNewEnvironmentUrl(value);
    if (!environmentTypeTouched) {
      setNewEnvironmentType(inferProjectEnvironmentFromUrl(value));
    }
  };

  const handleAddEnvironment = async () => {
    if (!projectId || !projectName || !newEnvironmentUrl.trim()) return;
    setAddingEnvironment(true);
    try {
      const normalizedUrl = normalizeProjectUrlInput(newEnvironmentUrl);
      await addEnvironmentUrl({
        projectId,
        url: normalizedUrl,
        label: `${projectName} (${formatEnvironmentLabel(newEnvironmentType)})`,
        environment: newEnvironmentType,
      });
      success("Environment added", `${normalizedUrl} is now available in the project switcher.`);
      setNewEnvironmentUrl("");
      setNewEnvironmentType("staging");
      setEnvironmentTypeTouched(false);
      await Promise.resolve(onProjectChanged?.());
    } catch (e) {
      showError("Could not add URL", String(e));
    } finally {
      setAddingEnvironment(false);
    }
  };

  const handleDeleteEnvironment = async (environment: EnvironmentRecord) => {
    if (projectEnvironments.length <= 1) return;
    setDeletingEnvironmentId(environment.id);
    try {
      await deleteEnvironment({ environmentId: environment.id });
      success("Environment removed", `${environment.url} was removed from this project.`);
      await Promise.resolve(onProjectChanged?.());
    } catch (e) {
      showError("Could not remove URL", String(e));
    } finally {
      setDeletingEnvironmentId(null);
    }
  };

  return (
    <div className="settings-section-stack">
      {view === "site-setup" && projectId ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Project Name</h2>
          </div>
          <div className="row-between">
            <div>
              <p className="text-13-medium">Display name</p>
              <p className="subtitle-xs">
                The name shown in the sidebar, project switcher, reports, and notifications.
              </p>
            </div>
            <div className="settings-field-row">
              <input
                type="text"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleRename()}
                className="field-control field-control--card field-control--compact settings-name-input"
              />
              <Button
                onClick={handleRename}
                disabled={renaming || !editName.trim() || editName.trim() === projectName}
                size="sm">
                {renaming ? "Saving…" : "Save"}
              </Button>
            </div>
          </div>
        </section>
      ) : null}

      {view === "site-setup" && projectId ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Project Folder</h2>
          </div>
          <p className="body-muted settings-card-intro">
            The local repo SiteCMD uses for Code Scan, dependency updates, deploy history, and
            launch guardrails.
          </p>
          <div className="row-between settings-well bg-card">
            <div className="settings-well-copy">
              <p className="text-13-medium">{projectPath ? "Linked folder" : "No folder linked"}</p>
              <p className="subtitle-xs settings-well-path">
                {projectPath ??
                  "Choose the project root that contains package.json, src, app, or config files."}
              </p>
              {framework ? (
                <p className="subtitle-xs settings-well-note">Detected framework: {framework}</p>
              ) : null}
            </div>
            <div className="settings-well-actions">
              <Button
                onClick={() => {
                  void handleChangeProjectPath();
                }}
                disabled={changingProjectPath || unlinkingProjectPath}
                size="sm">
                <FolderOpen />
                {changingProjectPath ? "Opening…" : projectPath ? "Change Folder" : "Select Folder"}
              </Button>
              {projectPath ? (
                <Button
                  onClick={() => {
                    void handleUnlinkProjectPath();
                  }}
                  disabled={changingProjectPath || unlinkingProjectPath}
                  variant="destructive"
                  size="sm">
                  <X />
                  {unlinkingProjectPath ? "Unlinking…" : "Unlink"}
                </Button>
              ) : null}
            </div>
          </div>
        </section>
      ) : null}

      {view === "site-setup" && projectId ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Site Environments</h2>
          </div>
          <p className="body-muted settings-card-intro">
            Add the production, staging, development, or local URLs SiteCMD should treat as the same
            project. These drive the environment switcher and future scans.
          </p>
          <div className="settings-list">
            {projectEnvironments.map((environment) => (
              <div key={environment.id} className="row-between settings-well bg-card">
                <div className="settings-well-copy">
                  <p className="text-13-medium">
                    {formatEnvironmentLabel(environment.environment)}
                  </p>
                  <p className="subtitle-xs settings-well-path">{environment.url}</p>
                </div>
                <Button
                  onClick={() => {
                    void handleDeleteEnvironment(environment);
                  }}
                  disabled={
                    projectEnvironments.length <= 1 || deletingEnvironmentId === environment.id
                  }
                  variant="destructive"
                  size="sm"
                  title={
                    projectEnvironments.length <= 1
                      ? "Each project needs at least one environment URL"
                      : "Remove this environment URL"
                  }>
                  {deletingEnvironmentId === environment.id ? "Removing…" : "Remove"}
                </Button>
              </div>
            ))}

            <div className="settings-add-env bg-card">
              <p className="text-13-medium settings-add-env-title">Add another environment</p>
              <div className="settings-add-env-row">
                <input
                  type="text"
                  value={newEnvironmentUrl}
                  onChange={(e) => handleNewEnvironmentUrlChange(e.target.value)}
                  placeholder="https://staging.example.com"
                  className="field-control field-control--muted settings-add-env-input"
                />
                <select
                  value={newEnvironmentType}
                  onChange={(e) => {
                    setNewEnvironmentType(e.target.value as ProjectEnvironment);
                    setEnvironmentTypeTouched(true);
                  }}
                  className="field-control field-control--muted field-control--select settings-add-env-select">
                  <option value="production">Production</option>
                  <option value="staging">Staging</option>
                  <option value="development">Development</option>
                  <option value="local">Local</option>
                </select>
                <Button
                  onClick={() => {
                    void handleAddEnvironment();
                  }}
                  disabled={addingEnvironment || !newEnvironmentUrl.trim()}>
                  {addingEnvironment ? "Adding…" : "Add URL"}
                </Button>
              </div>
              <p className="subtitle-xs settings-add-env-tip">
                Tip: SiteCMD will guess the environment from the URL, but you can override it before
                saving.
              </p>
            </div>
          </div>
        </section>
      ) : null}

      {view === "data" ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Local Database</h2>
          </div>
          <div className="settings-list">
            <div className="row-between">
              <span className="body-muted">Stored at</span>
              <span className="text-meta settings-db-path text-foreground">
                {dbInfoQuery.isPending ? (
                  <InlineSkeleton variant="line" width="lg" />
                ) : (
                  (dbInfoQuery.data?.path ?? "unknown")
                )}
              </span>
            </div>
            <div className="row-between">
              <span className="body-muted">Current size</span>
              <span className="row-title-md settings-db-size">
                {dbInfoQuery.isPending ? (
                  <InlineSkeleton variant="line-lg" width="sm" />
                ) : (
                  (dbInfoQuery.data?.size ?? "unknown")
                )}
              </span>
            </div>
          </div>
        </section>
      ) : null}

      {view === "data" ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Backup and Restore</h2>
          </div>
          <div className="settings-stack">
            <div className="row-between">
              <div>
                <p className="text-13-medium">Download backup</p>
                <p className="subtitle-xs">
                  Save projects, environments, scan history, and settings to a local file.
                </p>
              </div>
              <Button onClick={handleExport} disabled={exporting} variant="outline" size="sm">
                <Download /> {exporting ? "Exporting…" : "Export"}
              </Button>
            </div>

            <div className="subtle-divider-top settings-divided-row row-between">
              <div>
                <p className="text-13-medium">Restore backup</p>
                <p className="subtitle-xs">
                  Replace the current local database with a previous SiteCMD backup. Restart after
                  restoring.
                </p>
              </div>
              <Button onClick={handleImport} disabled={importing} variant="outline" size="sm">
                <Upload /> {importing ? "Importing…" : "Import"}
              </Button>
            </div>
          </div>
        </section>
      ) : null}

      {view === "data" ? (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Cleanup</h2>
          </div>
          <div className="settings-stack">
            <div className="row-between">
              <div>
                <p className="text-body settings-danger-label text-red-400">
                  Clear all scan history
                </p>
                <p className="subtitle-xs">
                  Remove saved scan runs and issue history for every project in this workspace.
                  Projects, URLs, and integrations stay.
                </p>
              </div>
              {confirmClear ? (
                <div className="settings-confirm-actions">
                  <Button
                    onClick={() => setConfirmClear(false)}
                    disabled={clearing}
                    variant="outline"
                    size="sm">
                    Cancel
                  </Button>
                  <Button
                    onClick={handleClear}
                    disabled={clearing}
                    variant="destructive"
                    size="sm"
                    className="btn--bold">
                    {clearing ? "Clearing…" : "Confirm"}
                  </Button>
                </div>
              ) : (
                <Button
                  onClick={() => setConfirmClear(true)}
                  variant="outline"
                  size="sm"
                  className="text-destructive settings-danger-btn">
                  <Trash2 /> Clear History
                </Button>
              )}
            </div>
          </div>
        </section>
      ) : null}
    </div>
  );
}

export function SiteSetupSection(props: Omit<DataSectionProps, "view">) {
  return <DataSection {...props} view="site-setup" />;
}

export function DeleteProjectCard({
  projectId,
  projectName,
  onProjectDeleted,
}: {
  projectId?: number;
  projectName?: string;
  onProjectDeleted?: () => void | Promise<unknown>;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [confirmText, setConfirmText] = useState("");
  const [deleting, setDeleting] = useState(false);
  const { success, error: showError } = useToast();

  if (!projectId) return null;

  // Name confirmation prevents a stale dialog from deleting a promoted project.
  const expectedName = (projectName ?? "").trim();
  const nameConfirmed = expectedName.length === 0 || confirmText.trim() === expectedName;

  const closeConfirm = () => {
    setConfirmDelete(false);
    setConfirmText("");
  };

  const handleDelete = async () => {
    if (!nameConfirmed || deleting) return;
    setDeleting(true);
    try {
      await deleteProject({ projectId });
      success("Project deleted", `"${projectName}" has been removed.`);
      await Promise.resolve(onProjectDeleted?.());
    } catch (e) {
      showError("Delete failed", String(e));
    } finally {
      setDeleting(false);
      closeConfirm();
    }
  };

  return (
    <div className="settings-delete-card bg-muted">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title settings-card-title-critical">Remove This Project</h2>
      </div>
      <div className="row-between">
        <div>
          <p className="text-13-medium">Delete project and its local history</p>
          <p className="subtitle-xs">
            Permanently removes "{projectName}" and all its scan history, environments, and
            settings.
          </p>
        </div>
        {!confirmDelete ? (
          <Button
            onClick={() => setConfirmDelete(true)}
            variant="destructive"
            size="sm"
            className="settings-delete-btn">
            <Trash2 /> Delete project
          </Button>
        ) : null}
      </div>
      {confirmDelete ? (
        <div className="settings-delete-confirm">
          <label className="subtitle-xs" htmlFor="delete-project-confirm-name">
            Type <span className="settings-confirm-name text-foreground">{projectName}</span> to
            confirm.
          </label>
          <div className="settings-confirm-actions">
            <Input
              id="delete-project-confirm-name"
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleDelete();
              }}
              placeholder={projectName}
              className="settings-delete-input"
              disabled={deleting}
              autoFocus
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
            />
            <Button onClick={closeConfirm} disabled={deleting} variant="outline" size="sm">
              Cancel
            </Button>
            <Button
              onClick={handleDelete}
              disabled={deleting || !nameConfirmed}
              variant="destructive"
              size="sm"
              className="btn--bold">
              {deleting ? "Deleting…" : "Delete permanently"}
            </Button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
