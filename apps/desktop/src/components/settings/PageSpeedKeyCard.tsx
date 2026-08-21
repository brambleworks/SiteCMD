import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { pagespeedApiKeyIsSet, setPagespeedApiKey } from "@/lib/commands";
import { Button } from "@/components/ui/button";
import { ExtLink } from "@/components/ui/external-link";
import { useToast } from "@/hooks/useToast";
import { queryKeys } from "@/lib/query/query-keys";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";

/** Optional keychain-backed PageSpeed API key; its value is never read into the UI. */
export function PageSpeedKeyCard() {
  const queryClient = useQueryClient();
  const [keyInput, setKeyInput] = useState("");
  const [saving, setSaving] = useState(false);
  const toast = useToast();
  const queryKey = queryKeys.settings.pagespeedKey();
  const keyQuery = useQuery({
    queryKey,
    queryFn: pagespeedApiKeyIsSet,
  });
  const hasKey = keyQuery.data ?? false;
  const loading = keyQuery.isPending;

  const save = async () => {
    const trimmed = keyInput.trim();
    if (!trimmed || saving) return;
    setSaving(true);
    try {
      await setPagespeedApiKey({ key: trimmed });
      queryClient.setQueryData(queryKey, true);
      setKeyInput("");
      toast.success("PageSpeed API key saved");
    } catch (error) {
      toast.error("Could not save key", String(error));
    } finally {
      setSaving(false);
    }
  };

  const remove = async () => {
    if (saving) return;
    setSaving(true);
    try {
      await setPagespeedApiKey({ key: "" });
      queryClient.setQueryData(queryKey, false);
      setKeyInput("");
      toast.success("PageSpeed API key removed");
    } catch (error) {
      toast.error("Could not remove key", String(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <section className="card card--spacious">
      <div className="settings-card-title-rule">
        <h2 className="settings-card-title">PageSpeed Insights</h2>
      </div>
      <p className="body-muted settings-card-intro">
        Web Vitals uses Google PageSpeed Insights. Without a key the public API is shared and
        rate-limited (you may see &ldquo;429&rdquo; errors). Add a free key for higher limits
        (25,000 runs/day). It is stored in your OS keychain and sent only to Google&rsquo;s
        PageSpeed API to authenticate your own requests, never to SiteCMD.
      </p>
      {loading ? (
        <LoadingRegion label="PageSpeed key status loading" className="stack-base">
          <Skeleton className="pagespeed-skeleton-label" />
          <Skeleton className="pagespeed-skeleton-input" />
          <Skeleton className="pagespeed-skeleton-button" />
        </LoadingRegion>
      ) : keyQuery.isError ? (
        <div role="alert" className="row-between">
          <p className="agent-handoff-error">The saved key status could not be read.</p>
          <Button variant="outline" size="sm" onClick={() => void keyQuery.refetch()}>
            Retry
          </Button>
        </div>
      ) : (
        <>
          <label className="section-label-mid-block" htmlFor="pagespeed-api-key">
            API key
          </label>
          <input
            id="pagespeed-api-key"
            type="password"
            autoComplete="off"
            value={keyInput}
            onChange={(event) => setKeyInput(event.target.value)}
            placeholder={
              hasKey ? "Saved - paste a new key to replace" : "Paste your PageSpeed API key"
            }
            className="field-control field-control--card"
            disabled={saving}
          />
          <div className="pagespeed-actions">
            <Button size="sm" onClick={save} disabled={!keyInput.trim() || saving}>
              {saving ? "Saving..." : hasKey ? "Replace key" : "Save key"}
            </Button>
            {hasKey ? (
              <Button variant="outline" size="sm" onClick={remove} disabled={saving}>
                Remove
              </Button>
            ) : null}
            <ExtLink
              href="https://developers.google.com/speed/docs/insights/v5/get-started#APIKey"
              className="text-body-muted text-primary pagespeed-key-link">
              Get a free key →
            </ExtLink>
          </div>
        </>
      )}
    </section>
  );
}
