import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { copyToClipboard } from "@/lib/clipboard";
import { getAgentToolManualConfig } from "@/lib/commands";
import { errorMessage } from "@/lib/error-message";
import {
  MANUAL_SETUP_EDITORS,
  MANUAL_SETUP_EDITOR_LABELS,
  buildManualSetupBlock,
  manualSetupAgentTool,
  toManualSetupEditor,
  type ManualSetupEditor,
} from "@/lib/agent-tool-manual-config";
import { queryKeys } from "@/lib/query/query-keys";

const COPIED_RESET_MS = 1500;

/** Copyable MCP config for every editor, detected or not. */
export function AgentToolManualSetup() {
  const [open, setOpen] = useState(false);
  return (
    <details
      className="agent-manual-setup"
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary className="agent-manual-setup-summary">Manual setup</summary>
      {open ? <ManualSetupBody /> : null}
    </details>
  );
}

function ManualSetupBody() {
  const [editor, setEditor] = useState<ManualSetupEditor>("claude-code");
  const [copied, setCopied] = useState(false);
  const tool = manualSetupAgentTool(editor);
  const configQuery = useQuery({
    queryKey: queryKeys.settings.agentToolManualConfig(tool),
    queryFn: () => getAgentToolManualConfig({ tool }),
  });
  const block = configQuery.data ? buildManualSetupBlock(editor, configQuery.data) : null;

  const handleCopy = async () => {
    if (!block) return;
    if (await copyToClipboard(block.body)) {
      setCopied(true);
      setTimeout(() => setCopied(false), COPIED_RESET_MS);
    }
  };

  return (
    <div className="agent-manual-setup-body">
      <p className="text-13-muted text-relaxed">
        Add SiteCMD to an editor yourself, including editors SiteCMD does not detect. Paste the
        block below into the file or terminal it names, then restart the editor.
      </p>
      <label className="form-label" htmlFor="manual-setup-editor">
        Editor
      </label>
      <select
        id="manual-setup-editor"
        className="compact-select-field control-well select-well"
        value={editor}
        onChange={(event) => {
          setEditor(toManualSetupEditor(event.target.value));
          setCopied(false);
        }}>
        {MANUAL_SETUP_EDITORS.map((option) => (
          <option key={option} value={option}>
            {MANUAL_SETUP_EDITOR_LABELS[option]}
          </option>
        ))}
      </select>
      {configQuery.isPending ? <p className="text-meta">Preparing the setup block.</p> : null}
      {configQuery.isError ? (
        <p className="agent-handoff-error">
          {errorMessage(configQuery.error) || "SiteCMD could not build the setup block."}
        </p>
      ) : null}
      {block ? (
        <>
          <p className="text-meta agent-manual-setup-location">{block.location}</p>
          <div className="black-code-panel">
            <pre className="compact-code-block agent-manual-setup-code">{block.body}</pre>
          </div>
          {block.note ? <p className="text-meta text-relaxed">{block.note}</p> : null}
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => void handleCopy()}
            aria-label={`Copy ${block.label} setup`}>
            {copied ? (
              <Check className="icon-sm text-score-excellent" aria-hidden="true" />
            ) : (
              <Copy className="icon-sm" aria-hidden="true" />
            )}
            {copied ? "Copied" : "Copy"}
          </Button>
        </>
      ) : null}
    </div>
  );
}
