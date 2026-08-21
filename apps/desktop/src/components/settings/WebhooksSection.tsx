import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { useToast } from "@/hooks/useToast";
import {
  deleteWebhookConfig,
  getWebhookConfigs,
  saveWebhookConfig,
  testWebhook,
} from "@/lib/commands";
import { queryKeys } from "@/lib/query/query-keys";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";

export function WebhooksSection({ projectId }: { projectId?: number }) {
  const queryClient = useQueryClient();
  const canLoad = projectId != null;
  const queryKey = queryKeys.settings.webhooks(projectId ?? 0);
  const webhooksQuery = useQuery({
    queryKey,
    queryFn: () => getWebhookConfigs({ projectId: projectId as number }),
    enabled: canLoad,
  });
  const webhooks = webhooksQuery.data ?? [];
  const [newUrl, setNewUrl] = useState("");
  const [newEvents, setNewEvents] = useState<Set<string>>(new Set(["scan_complete"]));
  const [newSecret, setNewSecret] = useState("");
  const [testing, setTesting] = useState<number | null>(null);
  const toast = useToast();

  const reloadWebhooks = () => queryClient.invalidateQueries({ queryKey });

  const handleAdd = async () => {
    const url = newUrl.trim();
    if (!projectId || !url) return;
    try {
      await saveWebhookConfig({
        projectId,
        url,
        events: JSON.stringify([...newEvents]),
        secret: newSecret || null,
        enabled: true,
      });
      setNewUrl("");
      setNewSecret("");
      setNewEvents(new Set(["scan_complete"]));
      await reloadWebhooks();
      toast.success("Webhook added");
    } catch (e) {
      toast.error("Failed to add webhook", String(e));
    }
  };

  const handleDelete = async (id: number) => {
    try {
      await deleteWebhookConfig({ id });
      await reloadWebhooks();
    } catch (e) {
      toast.error("Failed to delete", String(e));
    }
  };

  const handleTest = async (wh: (typeof webhooks)[0]) => {
    setTesting(wh.id);
    try {
      await testWebhook({ id: wh.id });
      toast.success("Test delivered", `Webhook sent to ${wh.url}`);
    } catch (e) {
      toast.error("Test failed", String(e));
    }
    setTesting(null);
  };

  const EVENT_OPTIONS = [
    { id: "scan_complete", label: "Scan finished" },
    { id: "score_drop", label: "Score dropped" },
    { id: "critical_issue", label: "Critical issue found" },
  ];

  if (!projectId) {
    return (
      <section className="card card--spacious">
        <p className="body-muted">Select a project to manage webhook delivery.</p>
      </section>
    );
  }

  if (webhooksQuery.isPending) {
    return (
      <LoadingRegion label="Webhooks loading state" className="card card--spacious">
        <div className="settings-card-title-rule">
          <Skeleton className="webhook-skeleton-title" />
        </div>
        <Skeleton className="webhook-skeleton-desc" />
        <Skeleton className="webhook-skeleton-input" />
      </LoadingRegion>
    );
  }

  if (webhooksQuery.isError) {
    return (
      <section className="card card--spacious" role="alert">
        <SettingsPanelHeader
          title="Webhooks"
          description="Send selected scan events to team tools or lightweight automation outside SiteCMD."
        />
        <p className="agent-handoff-error">Saved webhooks could not load.</p>
        <Button
          className="webhook-retry-btn"
          variant="outline"
          size="sm"
          onClick={() => void webhooksQuery.refetch()}>
          Retry
        </Button>
      </section>
    );
  }

  return (
    <section className="card card--spacious">
      <SettingsPanelHeader
        title="Webhooks"
        description="Send selected scan events to team tools or lightweight automation outside SiteCMD."
      />

      {webhooks.length > 0 && (
        <div className="webhook-list">
          {webhooks.map((wh) => (
            <div key={wh.id} className="settings-webhook-row">
              <span
                className={wh.enabled ? "status-dot-success" : "status-dot-info status-dot-dim"}
              />
              <span className="text-mono-sm text-truncate flex-fill">{wh.url}</span>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void handleTest(wh)}
                disabled={testing === wh.id}>
                {testing === wh.id ? "Sending..." : "Test"}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void handleDelete(wh.id)}>
                Remove
              </Button>
            </div>
          ))}
        </div>
      )}

      <div className="stack-base">
        <div className="row">
          <input
            value={newUrl}
            onChange={(e) => setNewUrl(e.target.value)}
            placeholder="Webhook endpoint URL"
            className="field-control field-control--card flex-fill text-body-muted"
          />
          <Button onClick={() => void handleAdd()} disabled={!newUrl.trim()} size="sm">
            Add Webhook
          </Button>
        </div>
        <div className="row">
          {EVENT_OPTIONS.map((opt) => (
            <Button
              key={opt.id}
              type="button"
              onClick={() => {
                const next = new Set(newEvents);
                if (next.has(opt.id)) next.delete(opt.id);
                else next.add(opt.id);
                setNewEvents(next);
              }}
              variant={newEvents.has(opt.id) ? "default" : "outline"}
              size="sm">
              {opt.label}
            </Button>
          ))}
        </div>
        <input
          value={newSecret}
          onChange={(e) => setNewSecret(e.target.value)}
          placeholder="Optional signing secret"
          className="field-control field-control--card text-body-muted"
        />
      </div>
    </section>
  );
}

function SettingsPanelHeader({
  title,
  description,
  badge,
}: {
  title: string;
  description: string;
  badge?: string;
}) {
  return (
    <div className="settings-panel-header">
      <div className="flex-fill">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">{title}</h2>
          {badge ? <span className="settings-panel-badge">{badge}</span> : null}
        </div>
        <p className="body-muted settings-card-desc">{description}</p>
      </div>
    </div>
  );
}
