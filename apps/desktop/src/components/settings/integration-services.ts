/** Category and unlock metadata per integration type */
export const SERVICE_META: Record<
  string,
  { category: string; unlocks: string; appearsIn: string }
> = {
  plausible: {
    category: "Analytics & Monitoring",
    unlocks: "Visitors, top pages, and traffic sources.",
    appearsIn: "Dashboard, Traffic",
  },
  googleanalytics: {
    category: "Analytics & Monitoring",
    unlocks: "GA4 traffic and audience context.",
    appearsIn: "Dashboard, Traffic",
  },
  cloudflare: {
    category: "Analytics & Monitoring",
    unlocks: "CDN, cache, and edge threat data.",
    appearsIn: "Dashboard, Security, Traffic",
  },
  uptimerobot: {
    category: "Analytics & Monitoring",
    unlocks: "Uptime and response-time history.",
    appearsIn: "Dashboard, Traffic",
  },
  googlesearchconsole: {
    category: "Search & SEO",
    unlocks: "Search clicks, impressions, and indexing changes.",
    appearsIn: "Dashboard, Search & SEO, Issues",
  },
  bingwebmaster: {
    category: "Search & SEO",
    unlocks: "Bing crawl and ranking signals.",
    appearsIn: "Search & SEO",
  },
  github: {
    category: "Deploys & CI",
    unlocks: "Deploys, CI runs, pull requests, and GitHub issue handoff.",
    appearsIn: "Dashboard, Deploys, Issues",
  },
  jira: {
    category: "Issue Trackers",
    unlocks: "Push issues into Jira and keep them tied to rescans.",
    appearsIn: "Issues",
  },
};

export const SERVICE_CATEGORIES = [
  "All",
  "Analytics & Monitoring",
  "Search & SEO",
  "Deploys & CI",
  "Issue Trackers",
];

export const SERVICE_CATEGORY_INFO: Record<string, string> = {
  "Analytics & Monitoring": "Traffic, uptime, and edge signals.",
  "Search & SEO": "Search visibility and crawl signals.",
  "Deploys & CI": "Deploy, CI, and repository workflow.",
  "Issue Trackers": "Send issues into your team's tracker.",
};

export interface IntegrationConfig {
  integrationType: string;
  apiKey: string | null;
  siteId: string | null;
  extra: unknown;
  enabled: boolean;
}

export interface IntegrationData {
  integrationType: string;
  data: Record<string, unknown>;
  fetchedAt: string;
  error: string | null;
}

export const SERVICES = [
  {
    type: "plausible",
    name: "Plausible Analytics",
    description:
      "Privacy-friendly website analytics. See visitors, pageviews, top pages, and traffic sources.",
    keyLabel: "API Key",
    siteIdLabel: "Site ID",
    siteIdPlaceholder: "mysite.com",
    setupUrl: "https://plausible.io/settings/api-keys",
    setupUrlLabel: "Open Plausible API Keys →",
    setupSteps: [
      "Click the link above to go directly to your Plausible API Keys page",
      'Click "New API Key" and name it "SiteCMD"',
      'Make sure only "Stats API" is selected',
      "Copy the key and paste it below - it won't be shown again",
    ],
    siteIdHelp:
      "Your site's domain as it appears in Plausible (e.g. mysite.com). One API key works for all your sites - only the Site ID changes per project.",
    docsUrl: "https://plausible.io/docs/stats-api",
  },
  {
    type: "cloudflare",
    name: "Cloudflare",
    description:
      "CDN and security analytics. See cache hit rate, bandwidth, requests, and threats blocked.",
    keyLabel: "API Token",
    siteIdLabel: "Zone ID or domain",
    siteIdPlaceholder: "e.g. example.com",
    setupUrl: "https://dash.cloudflare.com/profile/api-tokens",
    setupUrlLabel: "Open Cloudflare API Tokens →",
    setupSteps: [
      "Click the link above to create an API token",
      'Use the "Read analytics" template, or create a custom token with Analytics:Read and Zone:Read permission',
      "Paste your domain, or paste the Zone ID from the domain overview page",
      "Save the token and target below",
    ],
    siteIdHelp:
      "Use your domain, or the Zone ID found on the domain overview page in the Cloudflare dashboard.",
    docsUrl: "https://developers.cloudflare.com/analytics/",
  },
  {
    type: "uptimerobot",
    name: "UptimeRobot",
    description: "Uptime monitoring. See uptime percentage, response times, and downtime events.",
    keyLabel: "API Key",
    siteIdLabel: null,
    siteIdPlaceholder: null,
    setupUrl: "https://dashboard.uptimerobot.com/integrations",
    setupUrlLabel: "Open UptimeRobot API Settings →",
    setupSteps: [
      "Click the link above to go to your UptimeRobot integrations page",
      "Find your Main API Key (or create a Read-Only key for better security)",
      "Copy and paste it below - SiteCMD will automatically find monitors matching your site",
    ],
    siteIdHelp: null,
    docsUrl: "https://uptimerobot.com/api/",
  },
  {
    type: "bingwebmaster",
    name: "Bing Webmaster Tools",
    description:
      "Bing search performance - clicks, impressions, average position, top queries and pages.",
    keyLabel: "API Key",
    siteIdLabel: "Site URL",
    siteIdPlaceholder: "https://mysite.com",
    setupUrl: "https://www.bing.com/webmasters/",
    setupUrlLabel: "Open Bing Webmaster Tools →",
    setupSteps: [
      "Click the link above and sign in to Bing Webmaster Tools",
      "Add and verify your site if you haven't already",
      "Open Settings → API Access and copy your API key (one key works for all your sites)",
      "Paste the API key below and enter your site URL",
    ],
    siteIdHelp:
      "Enter the URL exactly as it appears in Bing Webmaster Tools (with https:// and trailing slash if shown).",
    docsUrl: "https://learn.microsoft.com/en-us/bingwebmaster/getting-access",
  },
] as const;

// OAuth-based services - "click to connect" flow
export const GITHUB_SERVICE = {
  type: "github",
  name: "GitHub",
  description: "CI/CD status, deployments, and open pull requests from your GitHub repository.",
  keyLabel: "API Token",
  siteIdLabel: "Repository Slug",
  siteIdPlaceholder: "owner/repository",
  setupUrl: "https://github.com/settings/tokens",
  setupUrlLabel: "Open GitHub token settings →",
  setupSteps: [
    "Create a GitHub personal access token with repository read access",
    "Enter the repository slug in owner/repository format",
    "Paste the token below so SiteCMD can pull deploys, CI runs, and pull request context",
  ],
  siteIdHelp:
    "Use the GitHub repository slug exactly as it appears on GitHub, for example owner/repository.",
  docsUrl: "https://docs.github.com/en/rest",
} as const;

// Jira - custom multi-field form (instance URL, email, API token, project key, issue type)
export const JIRA_SERVICE = {
  type: "jira",
  name: "Jira",
  description:
    "Push scan issues to Jira with fix details. Auto-resolves when issues pass on rescan.",
  setupUrl: "https://id.atlassian.com/manage-profile/security/api-tokens",
  setupUrlLabel: "Create API Token",
  setupSteps: [
    "Go to Atlassian Account → Security → API Tokens",
    "Click 'Create API token' and copy it",
    "Enter your Jira instance URL (e.g. yourcompany.atlassian.net)",
    "Enter the email for your Atlassian account",
    "Enter the project key (e.g. PROJ) and issue type",
  ],
  docsUrl:
    "https://support.atlassian.com/atlassian-account/docs/manage-api-tokens-for-your-atlassian-account/",
} as const;

/** Return the display name, falling back to the raw integration type. */
export function integrationDisplayName(type: string): string {
  return INTEGRATION_DISPLAY_NAMES[type] ?? type;
}

// Google services use OAuth, not API keys
export const GOOGLE_SERVICES = [
  {
    type: "googleanalytics",
    name: "Google Analytics (GA4)",
    description:
      "Full GA4 analytics - active users, sessions, pageviews, bounce rate, top pages and sources.",
  },
  {
    type: "googlesearchconsole",
    name: "Google Search Console",
    description:
      "Search performance - clicks, impressions, CTR, average position, top queries and pages.",
  },
] as const;

const INTEGRATION_DISPLAY_NAMES: Record<string, string> = Object.fromEntries(
  [...SERVICES, ...GOOGLE_SERVICES, GITHUB_SERVICE, JIRA_SERVICE].map((service) => [
    service.type,
    service.name,
  ]),
);
