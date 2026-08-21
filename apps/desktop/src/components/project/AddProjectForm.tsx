import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { FolderOpen, Plus, X, Loader2 } from "lucide-react";

import type { AddProjectFormProps } from "./add-project-form-model";
import { useAddProjectFormState } from "./useAddProjectFormState";

export function AddProjectForm({ onCreated, onCancel }: AddProjectFormProps) {
  const {
    addUrlRow,
    folder,
    folderError,
    folderNotice,
    framework,
    handleRemoveFolder,
    handleSelectFolder,
    handleSubmit,
    hasValidUrl,
    name,
    removeUrl,
    saving,
    scanning,
    setName,
    submitError,
    updateUrl,
    urls,
  } = useAddProjectFormState({ onCreated });

  return (
    <div className="add-project-form">
      <h2 className="add-project-title">Add a project</h2>
      <>
        <p className="text-body-muted add-project-subtitle">Add a site URL, a folder, or both.</p>

        <div className="add-project-fields">
          <div className="requirement-list">
            <div className="requirement-row">
              <label htmlFor="project-name" className="requirement-row__label">
                Project name
              </label>
              <Input
                id="project-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My Website"
                className="control-well"
                autoFocus
              />
            </div>
          </div>

          <div className="requirement-list">
            <div className="requirement-row">
              <label htmlFor="primary-url" className="requirement-row__label">
                Site URL
              </label>
              <div className="requirement-row__body">
                {urls.map((row, index) => (
                  <div key={index} className="row">
                    <select
                      value={row.environment}
                      onChange={(e) => updateUrl(index, "environment", e.target.value)}
                      className="compact-select-field control-well select-well"
                      aria-label={index === 0 ? "Primary environment" : `Environment ${index + 1}`}>
                      <option value="production">Production</option>
                      <option value="staging">Staging</option>
                      <option value="development">Development</option>
                      <option value="local">Local</option>
                    </select>
                    <Input
                      id={index === 0 ? "primary-url" : undefined}
                      aria-label={index === 0 ? undefined : `Environment ${index + 1} URL`}
                      value={row.url}
                      onChange={(e) => updateUrl(index, "url", e.target.value)}
                      placeholder={index === 0 ? "mysite.com" : "staging.mysite.com"}
                      className="control-well flex-fill"
                      autoCapitalize="off"
                      autoCorrect="off"
                      spellCheck={false}
                    />
                    {index > 0 ? (
                      <Button
                        unstyled
                        onClick={() => removeUrl(index)}
                        className="icon-remove-btn"
                        aria-label={`Remove environment ${index + 1}`}>
                        <X className="icon-sm" />
                      </Button>
                    ) : null}
                  </div>
                ))}
                <Button unstyled onClick={addUrlRow} className="inline-muted-button">
                  <Plus className="icon-xs" /> Add environment
                </Button>
              </div>
            </div>
          </div>

          <div>
            <div className="requirement-list">
              <div className="requirement-row">
                <span className="requirement-row__label">Source Code</span>
                <div className="requirement-row__body">
                  {folder ? (
                    <div className="control-well requirement-row__picker">
                      <FolderOpen className="icon-muted" />
                      <span className="flex-fill text-truncate">{folder}</span>
                      {framework && (
                        <span className="text-brand add-project-framework">{framework}</span>
                      )}
                      <Button
                        unstyled
                        onClick={handleRemoveFolder}
                        className="icon-remove-btn"
                        aria-label="Remove folder">
                        <X className="icon-sm" />
                      </Button>
                    </div>
                  ) : (
                    <Button
                      size="sm"
                      onClick={handleSelectFolder}
                      disabled={scanning}
                      className="btn--gap-tight">
                      {scanning ? (
                        <Loader2 className="icon-sm animate-spin" />
                      ) : (
                        <FolderOpen className="icon-sm" />
                      )}
                      {scanning ? "Scanning…" : "Select folder"}
                    </Button>
                  )}
                </div>
              </div>
            </div>
            {folderError ? (
              <p className="text-meta add-project-folder-note text-severity-medium">
                {folderError}
              </p>
            ) : null}
            {folderNotice ? (
              <p className="text-meta add-project-folder-note">{folderNotice}</p>
            ) : null}
          </div>

          {submitError ? (
            <p className="text-meta add-project-submit-note text-severity-medium" role="alert">
              {submitError}
            </p>
          ) : null}

          <div className="add-project-actions">
            <Button variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
            <Button
              onClick={handleSubmit}
              disabled={!name.trim() || (!folder && !hasValidUrl) || saving}>
              {saving ? "Creating…" : "Create Project"}
            </Button>
          </div>
        </div>
      </>
    </div>
  );
}
