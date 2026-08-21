import { useEffect, useRef } from "react";
import { IntegrationSettings } from "@/components/settings/IntegrationSettings";
import { PageSpeedKeyCard } from "@/components/settings/PageSpeedKeyCard";
import { SurfaceState } from "@/components/ui/surface-state";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { useIntegrationsQuery } from "@/hooks/useIntegrationsQuery";

interface IntegrationsPageProps {
  projectId: number;
  projectName: string;
  url: string;
  focusIntegration?: string | null;
  onFocusHandled?: () => void;
}

export function IntegrationsPage({
  projectId,
  projectName,
  url,
  focusIntegration,
  onFocusHandled,
}: IntegrationsPageProps) {
  const {
    configs,
    loading,
    error: loadError,
    reload: loadConfigs,
  } = useIntegrationsQuery(projectId);

  const onFocusHandledRef = useRef(onFocusHandled);
  useEffect(() => {
    onFocusHandledRef.current = onFocusHandled;
  }, [onFocusHandled]);

  useEffect(() => {
    if (loading || !focusIntegration) return;
    const selector = `[data-integration="${CSS.escape(focusIntegration)}"]`;
    const node = document.querySelector(selector);
    if (node instanceof HTMLElement) {
      node.scrollIntoView({ block: "center", behavior: "smooth" });
      node.classList.add("integration-focus-flash");
      window.setTimeout(() => node.classList.remove("integration-focus-flash"), 1500);
    }
    onFocusHandledRef.current?.();
  }, [focusIntegration, loading]);

  if (loadError) {
    return (
      <SurfaceState
        kind="error"
        title="Integrations could not load"
        description="We could not refresh the connection status for this project right now. Retry in a moment and SiteCMD will check again."
        className="page-content"
        primaryAction={{ label: "Retry", onClick: () => void loadConfigs() }}
      />
    );
  }

  if (loading) {
    return <IntegrationsLoadingState />;
  }

  return (
    <div className="page-content stack-hero">
      <IntegrationSettings
        projectId={projectId}
        projectName={projectName}
        url={url}
        focusIntegration={focusIntegration}
        configs={configs}
        onReloadConfigs={loadConfigs}
      />
      <PageSpeedKeyCard />
    </div>
  );
}

function IntegrationsLoadingState() {
  return (
    <LoadingRegion label="Integrations loading state" className="page-content stack-hero">
      <div className="stack-card">
        <div className="int-sk-head">
          <div className="stack-snug">
            <Skeleton className="int-sk-title" />
            <Skeleton className="int-sk-desc" />
          </div>
          <Skeleton className="int-sk-badge-sm" />
        </div>
        <div className="int-card-grid">
          {[0, 1].map((index) => (
            <div key={index} className="card card--spacious integration-card">
              <div className="int-card-head">
                <div className="row-start">
                  <Skeleton className="int-sk-icon" />
                  <div className="stack-snug">
                    <Skeleton className="int-sk-line-sm" />
                    <Skeleton className="int-sk-line-md" />
                  </div>
                </div>
                <Skeleton className="int-sk-pill" />
              </div>
              <Skeleton className="int-sk-body" />
              <Skeleton className="int-sk-body-2" />
              <Skeleton className="int-sk-body-3" />
              <Skeleton className="int-sk-btn" />
            </div>
          ))}
        </div>
      </div>

      <div className="stack-section">
        <div className="stack-snug">
          <Skeleton className="int-sk-title-lg" />
          <Skeleton className="int-sk-desc-lg" />
        </div>
        {[0, 1].map((group) => (
          <div key={group} className="stack-base">
            <div className="stack-snug">
              <Skeleton className="int-sk-group-title" />
              <Skeleton className="int-sk-group-desc" />
            </div>
            <div className="int-card-grid">
              {[0, 1, 2].map((index) => (
                <div key={index} className="card card--spacious integration-card">
                  <div className="int-card-head">
                    <div className="row-start">
                      <Skeleton className="int-sk-icon" />
                      <div className="stack-snug">
                        <Skeleton className="int-sk-line-sm" />
                        <Skeleton className="int-sk-line-md" />
                      </div>
                    </div>
                    <Skeleton className="int-sk-pill" />
                  </div>
                  <Skeleton className="int-sk-body" />
                  <Skeleton className="int-sk-body-2" />
                  <Skeleton className="int-sk-body-3" />
                  <Skeleton className="int-sk-btn" />
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </LoadingRegion>
  );
}
