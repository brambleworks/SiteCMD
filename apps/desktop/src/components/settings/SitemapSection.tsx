import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useSitemap } from "@/hooks/useSitemap";
import {
  Search,
  RefreshCw,
  Loader2,
  FileText,
  ExternalLink as ExternalLinkIcon,
  ChevronDown,
  ChevronRight,
  BookOpen,
} from "lucide-react";
import { ExtLink } from "@/components/ui/external-link";
import { Pager } from "@/components/ui/pager";
import { LoadingRegion, Skeleton } from "@/components/ui/skeleton";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { pageWindow } from "@/lib/pagination";

// A sitemap can list thousands of routes; the review list mounts one page of
// them and the pager reveals the rest, as the Issues list does.
const SITEMAP_PAGE_SIZE = 50;

interface SitemapSectionProps {
  siteUrl?: string;
  siteId?: number;
  projectId?: number;
  framework?: string;
}

export function SitemapSection({ siteUrl, siteId, projectId, framework }: SitemapSectionProps) {
  const sitemap = useSitemap(siteUrl, siteId, projectId);
  const isPreparing = Boolean(siteUrl && !sitemap.siteId);

  if (!siteUrl) {
    return (
      <div className="card card--spacious sitemap-empty text-muted-foreground">
        Select a project and environment to manage sitemap pages.
      </div>
    );
  }

  if (sitemap.loading) {
    return (
      <LoadingRegion
        label="Sitemap settings loading state"
        className="card card--spacious sitemap-loading">
        <div className="settings-card-title-rule">
          <Skeleton variant="title" width="md" />
          <Skeleton variant="button" width="sm" />
        </div>
        <Skeleton variant="line" width="full" />
        <Skeleton variant="line" width="wide" />
      </LoadingRegion>
    );
  }

  return (
    <div className="sitemap-card-body">
      {(sitemap.state === "idle" || sitemap.state === "discovering") && (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">Find Pages From Sitemap</h2>
            <Button
              size="sm"
              onClick={sitemap.discover}
              disabled={sitemap.state === "discovering" || isPreparing}>
              {isPreparing ? (
                <>
                  <Loader2 className="icon-sm animate-spin" /> Preparing...
                </>
              ) : sitemap.state === "discovering" ? (
                <>
                  <Loader2 className="icon-sm animate-spin" /> Discovering…
                </>
              ) : (
                <>
                  <Search className="icon-sm" /> Find Pages
                </>
              )}
            </Button>
          </div>
          <p className="body-muted">
            SiteCMD can use your sitemap to understand which pages belong to this project. It checks
            common sitemap locations and robots.txt.
          </p>
        </section>
      )}

      {sitemap.state === "found" && sitemap.pages.length > 0 && (
        <PagesList
          pages={sitemap.pages}
          sourceUrl={sitemap.sourceUrl}
          onRefresh={sitemap.refresh}
        />
      )}

      {sitemap.state === "found" && sitemap.pages.length === 0 && (
        <section className="card card--spacious">
          <div className="settings-card-title-rule">
            <h2 className="settings-card-title">No Pages Saved Yet</h2>
          </div>
          <p className="body-muted">
            SiteCMD reached the sitemap flow, but no page URLs were stored. Try discovery again or
            enter a sitemap URL manually.
          </p>
          <div className="row-wrap sitemap-actions">
            <Button size="sm" onClick={sitemap.refresh}>
              <RefreshCw className="icon-sm" /> Refresh
            </Button>
            <Button size="sm" variant="outline" onClick={sitemap.showManualEntry}>
              <FileText className="icon-sm" /> Enter URL
            </Button>
          </div>
        </section>
      )}

      {sitemap.state === "not_found" && (
        <NotFoundCard
          error={sitemap.error}
          onManualEntry={sitemap.showManualEntry}
          onNoSitemap={sitemap.showNoSitemap}
          onRetry={sitemap.discover}
        />
      )}

      {sitemap.state === "error" && (
        <section className="card card--spacious">
          <div className="sitemap-card-body">
            <div className="settings-card-title-rule">
              <h2 className="settings-card-title settings-card-title-critical">
                Could Not Find Pages
              </h2>
            </div>
            <p className="body-muted">{sitemap.error}</p>
            <Button size="sm" variant="outline" onClick={sitemap.discover}>
              <RefreshCw className="icon-sm" /> Retry
            </Button>
          </div>
        </section>
      )}

      {sitemap.state === "manual_entry" && (
        <ManualEntryCard
          onSubmit={sitemap.submitManualUrl}
          onBack={() => sitemap.reset()}
          error={sitemap.error}
        />
      )}

      {sitemap.state === "no_sitemap" && (
        <CreationGuide framework={framework} onBack={() => sitemap.reset()} />
      )}
    </div>
  );
}

function NotFoundCard({
  error,
  onManualEntry,
  onNoSitemap,
  onRetry,
}: {
  error: string | null;
  onManualEntry: () => void;
  onNoSitemap: () => void;
  onRetry: () => void;
}) {
  return (
    <section className="card card--spacious">
      <div className="sitemap-card-body">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">No Sitemap Found</h2>
        </div>
        <p className="body-muted">
          {error || "SiteCMD could not find a sitemap.xml at common locations or in robots.txt."}
        </p>

        <div className="sitemap-guide-actions">
          <Button size="sm" className="btn--start" variant="outline" onClick={onManualEntry}>
            <FileText className="icon-sm" /> Enter sitemap URL
          </Button>
          <Button size="sm" className="btn--start" variant="outline" onClick={onNoSitemap}>
            <BookOpen className="icon-sm" /> Setup steps
          </Button>
          <Button size="sm" className="btn--start" variant="ghost" onClick={onRetry}>
            <RefreshCw className="icon-sm" />
            Try again
          </Button>
        </div>
      </div>
    </section>
  );
}

function ManualEntryCard({
  onSubmit,
  onBack,
  error,
}: {
  onSubmit: (url: string) => void;
  onBack: () => void;
  error: string | null;
}) {
  const [url, setUrl] = useState("");

  return (
    <section className="card card--spacious">
      <div className="sitemap-card-body">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Use a Specific Sitemap</h2>
        </div>
        <p className="body-muted">Paste the exact sitemap URL if your site uses a custom path.</p>
        <div className="sitemap-input-row">
          <Input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://example.com/sitemap.xml"
            autoCapitalize="off"
            autoCorrect="off"
            spellCheck={false}
          />
          <Button
            size="sm"
            onClick={() => url.trim() && onSubmit(url.trim())}
            disabled={!url.trim()}>
            Fetch
          </Button>
        </div>
        {error ? <p className="sitemap-error text-severity-critical">{error}</p> : null}
        <Button type="button" onClick={onBack} variant="ghost" size="sm">
          Back
        </Button>
      </div>
    </section>
  );
}

function CreationGuide({ framework, onBack }: { framework?: string; onBack: () => void }) {
  const guide = getCreationGuide(framework);

  return (
    <section className="card card--spacious">
      <div className="sitemap-card-body">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">Add a Sitemap to This Site</h2>
        </div>

        <div className="settings-guide-block">
          <p className="row-title-md">{guide.title}</p>
          <div className="sitemap-guide-steps">
            {guide.steps.map((step, i) => (
              <div key={i} className="settings-guide-step">
                <span className="settings-guide-number">{i + 1}</span>
                <span>{step}</span>
              </div>
            ))}
          </div>
          {guide.link ? (
            <ExtLink href={guide.link.url} className="settings-guide-link">
              {guide.link.label} <ExternalLinkIcon className="icon-xs" />
            </ExtLink>
          ) : null}
        </div>

        <Button onClick={onBack} variant="ghost" size="sm">
          Back
        </Button>
      </div>
    </section>
  );
}

function PagesList({
  pages,
  sourceUrl,
  onRefresh,
}: {
  pages: { id: number; url: string; path: string; source: string }[];
  sourceUrl: string | null;
  onRefresh: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(1);

  const filtered = search
    ? pages.filter(
        (p) =>
          p.path.toLowerCase().includes(search.toLowerCase()) ||
          p.url.toLowerCase().includes(search.toLowerCase()),
      )
    : pages;
  // A new search changes what the first page holds.
  useResetOnChange(search, () => setPage(1));
  const pagedRows = pageWindow(filtered, page, SITEMAP_PAGE_SIZE);

  return (
    <section className="card card--spacious">
      <div className="sitemap-card-body">
        <div className="settings-card-title-rule">
          <h2 className="settings-card-title">
            {pages.length} page{pages.length !== 1 ? "s" : ""} found
          </h2>
          <Button size="sm" variant="ghost" onClick={onRefresh}>
            <RefreshCw className="icon-xs" /> Refresh
          </Button>
        </div>

        <div className="settings-context-strip">
          <div className="tile">
            <p className="stat-label">Tracked Pages</p>
            <p className="settings-context-value">{pages.length}</p>
          </div>
          <div className="tile sitemap-source-tile">
            <p className="stat-label">Sitemap Source</p>
            <p className="settings-context-value settings-context-mono">
              {sourceUrl ?? "Saved sitemap"}
            </p>
          </div>
        </div>

        <Button type="button" onClick={() => setExpanded(!expanded)} variant="outline" size="sm">
          {expanded ? <ChevronDown className="icon-xs" /> : <ChevronRight className="icon-xs" />}
          {expanded ? "Hide Pages" : "Review Pages"}
        </Button>

        {expanded && (
          <div className="sitemap-expanded">
            {pages.length > 10 ? (
              <Input
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Search pages…"
                className="sitemap-search"
              />
            ) : null}
            <div className="settings-page-list">
              {pagedRows.rows.map((row) => (
                <div key={row.id} className="settings-page-row">
                  <FileText className="icon-xs text-muted-foreground" />
                  <span className="sitemap-page-path text-muted-foreground">{row.path || "/"}</span>
                </div>
              ))}
              {search && filtered.length === 0 ? (
                <p className="muted-text sitemap-no-match">No pages match "{search}"</p>
              ) : null}
            </div>
            <Pager
              page={pagedRows.page}
              totalPages={pagedRows.totalPages}
              onChange={setPage}
              label="Sitemap pages"
              itemLabel="sitemap"
            />
          </div>
        )}
      </div>
    </section>
  );
}

function getCreationGuide(framework?: string) {
  const lower = (framework || "").toLowerCase();

  if (lower.includes("drupal")) {
    return {
      title: "Drupal Sitemap",
      steps: [
        "Install the Simple XML Sitemap module: composer require drupal/simple_sitemap",
        "Enable it: drush en simple_sitemap -y",
        "Go to Configuration → Search and metadata → Simple XML Sitemap",
        "Select which content types and menus to include",
        "Your sitemap will be available at /sitemap.xml",
      ],
      link: {
        label: "Simple XML Sitemap docs",
        url: "https://www.drupal.org/project/simple_sitemap",
      },
    };
  }

  if (lower.includes("wordpress") || lower.includes("wp")) {
    return {
      title: "WordPress Sitemap",
      steps: [
        "WordPress 5.5+ includes a built-in sitemap at /wp-sitemap.xml",
        "For more control, install Yoast SEO or Rank Math plugin",
        "Go to SEO → General → Features and enable XML Sitemaps",
        "Your sitemap will be at /sitemap_index.xml",
      ],
      link: {
        label: "Yoast SEO Sitemaps",
        url: "https://yoast.com/help/xml-sitemaps-in-the-yoast-seo-plugin/",
      },
    };
  }

  if (lower.includes("next") || lower.includes("react")) {
    return {
      title: "Next.js Sitemap",
      steps: [
        "Install next-sitemap: npm install next-sitemap",
        "Create next-sitemap.config.js in your project root",
        "Set siteUrl to your production URL",
        "Add 'postbuild': 'next-sitemap' to package.json scripts",
        "Run the build - sitemap.xml will be generated in /public",
      ],
      link: { label: "next-sitemap on npm", url: "https://www.npmjs.com/package/next-sitemap" },
    };
  }

  if (lower.includes("hugo")) {
    return {
      title: "Hugo Sitemap",
      steps: [
        "Hugo generates sitemap.xml automatically on build",
        "Check your config.toml/yaml for [sitemap] settings",
        "Run 'hugo' to build - sitemap.xml is in /public/",
        "Deploy and verify at your-site.com/sitemap.xml",
      ],
      link: {
        label: "Hugo Sitemap Template",
        url: "https://gohugo.io/templates/sitemap-template/",
      },
    };
  }

  if (lower.includes("jekyll")) {
    return {
      title: "Jekyll Sitemap",
      steps: [
        "Add gem 'jekyll-sitemap' to your Gemfile",
        "Run 'bundle install'",
        "Add jekyll-sitemap to the plugins list in _config.yml",
        "Build your site - sitemap.xml will be generated automatically",
      ],
      link: { label: "jekyll-sitemap plugin", url: "https://github.com/jekyll/jekyll-sitemap" },
    };
  }

  if (lower.includes("laravel")) {
    return {
      title: "Laravel Sitemap",
      steps: [
        "Install spatie/laravel-sitemap: composer require spatie/laravel-sitemap",
        "Create a command or route to generate the sitemap",
        "Use Sitemap::create(url)->writeToFile(public_path('sitemap.xml'))",
        "Schedule the generation or run it as part of deployment",
      ],
      link: { label: "spatie/laravel-sitemap", url: "https://github.com/spatie/laravel-sitemap" },
    };
  }

  return {
    title: "Create a Sitemap",
    steps: [
      "A sitemap.xml tells search engines about all pages on your site",
      "Create an XML file at your-site.com/sitemap.xml",
      "List each page URL inside <urlset><url><loc>...</loc></url></urlset> tags",
      "Add a Sitemap: directive to your robots.txt pointing to the sitemap",
      "Many CMS platforms and frameworks can generate this automatically",
    ],
    link: { label: "sitemaps.org protocol", url: "https://www.sitemaps.org/protocol.html" },
  };
}
