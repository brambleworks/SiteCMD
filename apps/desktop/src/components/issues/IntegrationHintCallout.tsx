import { Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { IntegrationSuggestion } from "@/lib/types";
import { dismissIntegrationHint } from "@/lib/issues";

const INTEGRATION_LABELS: Record<string, string> = {
  plausible: "Plausible",
  cloudflare: "Cloudflare",
  uptimerobot: "UptimeRobot",
  googleanalytics: "Google Analytics",
  googlesearchconsole: "Search Console",
  bingwebmaster: "Bing Webmaster",
  github: "GitHub",
  jira: "Jira",
};

interface Props {
  projectId: number;
  suggestions: IntegrationSuggestion[];
  onOpenIntegrations?: (integration: string) => void;
  onDismissed?: (checkId: string, integration: string) => void;
}

export function IntegrationHintCallout({
  projectId,
  suggestions,
  onOpenIntegrations,
  onDismissed,
}: Props) {
  if (suggestions.length === 0) return null;
  return (
    <div className="stack-snug">
      {suggestions.map((s) => {
        const label = INTEGRATION_LABELS[s.integration] ?? s.integration;
        return (
          <div className="callout-integration-hint" key={`${s.checkId}:${s.integration}`}>
            <Sparkles className="callout-integration-hint-icon icon-sm text-primary" />
            <div className="flex-fill">
              <div className="callout-integration-hint-title">Get more context</div>
              <div className="callout-integration-hint-body">{s.valueProp}</div>
              <div className="callout-integration-hint-actions">
                <Button
                  variant="default"
                  size="sm"
                  onClick={() => onOpenIntegrations?.(s.integration)}>
                  Connect {label} →
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={async () => {
                    await dismissIntegrationHint(projectId, s.checkId, s.integration);
                    onDismissed?.(s.checkId, s.integration);
                  }}>
                  Dismiss
                </Button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
