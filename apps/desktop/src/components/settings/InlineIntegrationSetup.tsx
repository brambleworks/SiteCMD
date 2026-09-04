import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ChevronDown, ExternalLink as ExternalLinkIcon, Loader2 } from "lucide-react";
import { getHostname } from "@/lib/utils";
import {
  SERVICES,
  GOOGLE_SERVICES,
  GITHUB_SERVICE,
} from "@/components/settings/integration-services";
import { ServiceIconWithBg } from "@/components/icons/ServiceIcon";
import { ExtLink } from "@/components/ui/external-link";
import { GoogleSignInButton } from "@/components/ui/google-sign-in-button";
import { useInlineIntegrationSetupState } from "@/components/settings/useInlineIntegrationSetupState";
import { GooglePicker } from "@/components/settings/IntegrationGooglePicker";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";

interface InlineIntegrationSetupProps {
  serviceTypes: string[];
  projectId: number;
  url?: string;
  onConnected?: (type: string) => void;
  includeGoogle?: boolean;
  compact?: boolean;
  /** Services that remain available for reconnection even when configured. */
  allowReconnect?: string[];
}

export function InlineIntegrationSetup({
  serviceTypes,
  projectId,
  url,
  onConnected,
  includeGoogle = false,
  compact = false,
  allowReconnect = [],
}: InlineIntegrationSetupProps) {
  const {
    apiKey,
    configs,
    configsLoading,
    closeGooglePicker,
    expandedService,
    ghConnecting,
    ghDeviceCode,
    ghRepos,
    googleConnecting,
    googleError,
    googlePickerData,
    googlePickerTarget,
    handleGitHubConnect,
    handleGoogleConnect,
    handlePickGitHubRepo,
    handlePickGoogleProperty,
    handleSave,
    saving,
    setApiKey,
    setExpandedService,
    setGhRepos,
    setSiteId,
    siteId,
    toggleApiService,
  } = useInlineIntegrationSetupState({ onConnected, projectId, url });

  if (configsLoading) {
    return (
      <LoadingRegion label="Integration options loading" className="stack-base">
        {[0, 1].map((index) => (
          <div key={index} className="card inline-int-pad">
            <div className="inline-int-service-row">
              <Skeleton className="inline-int-sk-icon" />
              <div className="flex-fill stack-snug">
                <Skeleton className="inline-int-sk-name" />
                <Skeleton className="inline-int-sk-desc" />
              </div>
              <Skeleton className="inline-int-sk-btn" />
            </div>
          </div>
        ))}
      </LoadingRegion>
    );
  }

  const connectedTypes = new Set(configs.map((c) => c.integrationType));
  // A reconnectable type is shown even when already configured, so a broken
  // integration (e.g. an expired OAuth token) can be re-authed from its card.
  const reconnectable = new Set(allowReconnect);
  const isShowable = (type: string) => !connectedTypes.has(type) || reconnectable.has(type);

  const apiServices = SERVICES.filter((s) => serviceTypes.includes(s.type));
  const googleServices = includeGoogle
    ? GOOGLE_SERVICES.filter((s) => serviceTypes.includes(s.type))
    : [];

  // Show services that aren't connected yet, plus any flagged for reconnect.
  const unconnectedApiAccessible = apiServices.filter((s) => isShowable(s.type));
  const unconnectedGoogleAccessible = googleServices.filter((s) => isShowable(s.type));
  const googleTargetType =
    unconnectedGoogleAccessible.length === 1 ? unconnectedGoogleAccessible[0]?.type : undefined;
  const showGitHub = serviceTypes.includes("github") && isShowable("github");
  const accessibleCount =
    unconnectedApiAccessible.length + unconnectedGoogleAccessible.length + (showGitHub ? 1 : 0);

  // Nothing to show if everything's connected
  if (accessibleCount === 0) return null;

  return (
    <div className="stack-base">
      {unconnectedGoogleAccessible.length > 0 && !googlePickerData && (
        <div className="card">
          <div className="inline-int-pad">
            <div className="inline-int-service-row">
              <ServiceIconWithBg type="googleanalytics" />
              <div className="flex-fill">
                <span className="inline-int-name">
                  {unconnectedGoogleAccessible.map((s) => s.name).join(" & ")}
                </span>
                <p className="muted-text inline-int-sub">
                  SiteCMD has read-only access to your Analytics and Search Console data and keeps
                  it on your device.{" "}
                  <ExtLink href="https://sitecmd.com/privacy">Privacy Policy</ExtLink>
                </p>
              </div>
              <GoogleSignInButton
                onClick={() => handleGoogleConnect(googleTargetType)}
                loading={googleConnecting}
              />
            </div>
            {googleError ? <p className="text-body inline-int-error">{googleError}</p> : null}
          </div>
        </div>
      )}

      {googlePickerData && (
        <GooglePicker
          data={googlePickerData}
          connectedTypes={connectedTypes}
          projectHost={url ? getHostname(url) : ""}
          targetType={googlePickerTarget}
          onPick={handlePickGoogleProperty}
          onClose={closeGooglePicker}
        />
      )}

      {unconnectedApiAccessible.map((service) => {
        const isExpanded = expandedService === service.type;

        return (
          <div key={service.type} className="card">
            <div className="inline-int-flush">
              <Button
                unstyled
                onClick={() => {
                  toggleApiService(service.type, Boolean(service.siteIdLabel));
                }}
                className="inline-integration-row">
                <ServiceIconWithBg type={service.type} />
                <div className="flex-fill">
                  <span className="inline-int-name">{service.name}</span>
                  {compact ? (
                    <p className="muted-text inline-int-sub">Connect to track {service.name}.</p>
                  ) : (
                    <p className="muted-text inline-int-sub">{service.description}</p>
                  )}
                </div>
                {isExpanded ? (
                  <ChevronDown className="icon-md text-muted-foreground" />
                ) : (
                  <span className="btn btn--outline btn--sm inline-int-connect-tag">Connect</span>
                )}
              </Button>

              {isExpanded && (
                <div className="inline-int-expand stack-card">
                  <div>
                    <p className="text-meta inline-int-howto">How to connect</p>
                    {"setupUrl" in service && service.setupUrl && (
                      <ExtLink href={service.setupUrl as string} className="inline-int-setup-link">
                        {"setupUrlLabel" in service
                          ? (service.setupUrlLabel as string)
                          : "Open setup page"}{" "}
                        <ExternalLinkIcon className="icon-xs" />
                      </ExtLink>
                    )}
                    <ol className="inline-int-steps">
                      {service.setupSteps.map((step, i) => (
                        <li key={i} className="muted-text inline-int-step">
                          <span className="inline-int-step-num">{i + 1}.</span>
                          <span>{step}</span>
                        </li>
                      ))}
                    </ol>
                  </div>

                  <div className="stack-snug">
                    <div>
                      <label className="inline-int-label">{service.keyLabel}</label>
                      <Input
                        type="password"
                        value={apiKey}
                        onChange={(e) => setApiKey(e.target.value)}
                        placeholder={`Paste your ${service.keyLabel.toLowerCase()} here`}
                        className="inline-int-input"
                        autoCapitalize="off"
                        autoCorrect="off"
                        spellCheck={false}
                      />
                    </div>
                    {service.siteIdLabel && (
                      <div>
                        <label className="inline-int-label">{service.siteIdLabel}</label>
                        {service.siteIdHelp && <p className="subtitle-xs">{service.siteIdHelp}</p>}
                        <Input
                          value={siteId}
                          onChange={(e) => setSiteId(e.target.value)}
                          placeholder={service.siteIdPlaceholder || ""}
                          className="inline-int-input"
                          autoCapitalize="off"
                          autoCorrect="off"
                        />
                      </div>
                    )}
                  </div>

                  <div className="row">
                    <Button onClick={() => handleSave(service.type)} disabled={!apiKey || saving}>
                      {saving ? "Connecting…" : "Connect"}
                    </Button>
                    <Button variant="ghost" onClick={() => setExpandedService(null)}>
                      Cancel
                    </Button>
                  </div>
                </div>
              )}
            </div>
          </div>
        );
      })}

      {showGitHub && !ghRepos && (
        <div className="card">
          <div className="inline-int-pad stack-base">
            <div className="inline-int-service-row">
              <ServiceIconWithBg type="github" />
              <div className="flex-fill">
                <span className="inline-int-name">{GITHUB_SERVICE.name}</span>
                {!compact && (
                  <p className="muted-text inline-int-sub">{GITHUB_SERVICE.description}</p>
                )}
              </div>
              <Button size="sm" onClick={handleGitHubConnect} disabled={ghConnecting}>
                {ghConnecting ? (
                  <>
                    <Loader2 className="icon-sm inline-int-spinner animate-spin" />
                    Waiting…
                  </>
                ) : (
                  "Connect with GitHub"
                )}
              </Button>
            </div>
            {ghDeviceCode && (
              <div className="inline-int-code-box">
                <p className="subtitle-xs inline-int-code-label">
                  Enter this code in the GitHub tab we opened:
                </p>
                <div className="inline-int-code-row">
                  <span className="inline-int-code">{ghDeviceCode.userCode}</span>
                  <ExtLink href={ghDeviceCode.verificationUri} className="inline-int-link">
                    Open GitHub
                  </ExtLink>
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {ghRepos && (
        <div className="card inline-int-picker-card">
          <div className="inline-int-pad stack-card">
            <p className="inline-int-picker-title">Choose a repository</p>
            <div className="inline-int-repo-list">
              {ghRepos.map((repo) => (
                <Button
                  unstyled
                  key={repo.full_name}
                  onClick={() => handlePickGitHubRepo(repo.full_name)}
                  className="inline-integration-choice">
                  <div className="flex-fill">
                    <span className="inline-int-repo-name">{repo.full_name}</span>
                    {repo.private && (
                      <span className="subtitle-xs inline-int-repo-tag">private</span>
                    )}
                    {repo.description && (
                      <p className="muted-text text-truncate inline-int-sub">{repo.description}</p>
                    )}
                  </div>
                  <span className="muted-text text-mono inline-int-repo-tag">
                    {repo.default_branch}
                  </span>
                </Button>
              ))}
            </div>
            <Button variant="ghost" size="sm" onClick={() => setGhRepos(null)}>
              Cancel
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
