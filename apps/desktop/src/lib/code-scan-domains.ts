import {
  Database,
  Bot,
  Shield,
  Layers,
  Activity,
  Package,
  FileCog,
  type LucideIcon,
} from "lucide-react";
import type { CodeIssue, CodeScanDomain } from "@/lib/types";

export type { CodeScanDomain } from "@/lib/types";

interface CodeScanDomainMeta {
  label: string;
  shortLabel: string;
  description: string;
  icon: LucideIcon;
  /** CSS variable name (bare, no `var`) for the domain accent color. */
  accentVar: string;
}

export const CODE_SCAN_DOMAIN_ORDER: CodeScanDomain[] = [
  "database",
  "ai-safety",
  "security",
  "architecture",
  "operations",
  "supply-chain",
  "ai-scaffolding",
];

export const CODE_SCAN_DOMAIN_META: Record<CodeScanDomain, CodeScanDomainMeta> = {
  database: {
    label: "Database Analysis",
    shortLabel: "Database",
    description:
      "Schema drift, migrations, query safety, Supabase policies, and local DB integrity checks.",
    icon: Database,
    accentVar: "--cat-code",
  },
  "ai-safety": {
    label: "AI Safety",
    shortLabel: "AI",
    description:
      "Timeouts, retries, quotas, spend guardrails, loops, and rollout safety for AI features.",
    icon: Bot,
    accentVar: "--cat-code",
  },
  security: {
    label: "Security",
    shortLabel: "Security",
    description:
      "Auth, permissions, request validation, sanitization, SSRF, and other code-level exposure risks.",
    icon: Shield,
    accentVar: "--cat-security",
  },
  architecture: {
    label: "Architecture",
    shortLabel: "Architecture",
    description:
      "God routes, missing shared layers, brittle structure, and hidden coupling that slows fixes down.",
    icon: Layers,
    accentVar: "--cat-code",
  },
  operations: {
    label: "Operations",
    shortLabel: "Ops",
    description:
      "Env drift, deploy safety, recovery notes, observability, and background job reliability.",
    icon: Activity,
    accentVar: "--cat-code",
  },
  "supply-chain": {
    label: "Dependencies",
    shortLabel: "Dependencies",
    description:
      "Dependency drift, suspicious packages, registry mismatches, and lockfile hygiene.",
    icon: Package,
    accentVar: "--cat-code",
  },
  "ai-scaffolding": {
    label: "AI Setup",
    shortLabel: "AI Setup",
    description:
      "Quality and consistency of your AI agent instruction files (CLAUDE.md, AGENTS.md, Cursor and Windsurf rules) and MCP server configs.",
    icon: FileCog,
    accentVar: "--cat-code",
  },
};

const DATABASE_ID_PREFIXES = [
  "local-db-target-remote",
  "local-sqlite-",
  "local-postgres-",
  "local-prisma-",
  "local-drizzle-",
  "supabase-",
  "schema-join-",
  "schema-relation-",
  "db-index-hints-",
  "db-scattered-across-routes",
  "unsafe-raw-sql",
  "interpolated-sql",
  "formatted-sql",
];

const DATABASE_HINTS = [
  "database",
  "sqlite",
  "postgres",
  "mysql",
  "supabase",
  "prisma",
  "drizzle",
  "migration",
  "schema",
  "column drift",
  "foreign key",
  "unique constraint",
  "row level security",
  "rls",
  "sql ",
  "query ",
  "transaction",
  "seed workflow",
];

// Domain derivation also accepts legacy issues without a stamped domain.
export type ClassifiableCodeIssue = Omit<CodeIssue, "domain"> & {
  domain?: CodeScanDomain | null;
};

function looksLikeDatabaseIssue(issue: ClassifiableCodeIssue): boolean {
  if (issue.category === "data") return true;
  if (DATABASE_ID_PREFIXES.some((prefix) => issue.id.startsWith(prefix))) return true;

  const haystack = [
    issue.id,
    issue.title,
    issue.description,
    issue.evidence,
    issue.whyNow,
    issue.likelyFix,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();

  return DATABASE_HINTS.some((hint) => haystack.includes(hint));
}

export function getCodeIssueDomain(issue: ClassifiableCodeIssue): CodeScanDomain {
  if (issue.domain) {
    return issue.domain;
  }
  if (issue.category === "ai-scaffolding") {
    return "ai-scaffolding";
  }
  if (issue.category === "ai-safety" || issue.id.startsWith("ai-")) {
    return "ai-safety";
  }
  if (looksLikeDatabaseIssue(issue)) {
    return "database";
  }
  if (issue.category === "security") {
    return "security";
  }
  if (issue.category === "supply-chain") {
    return "supply-chain";
  }
  if (issue.category === "operations") {
    return "operations";
  }
  return "architecture";
}
