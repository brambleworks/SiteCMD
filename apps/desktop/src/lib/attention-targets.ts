import { getOpenTargetLabel } from "@/lib/action-language";
import type { AppTarget } from "@/lib/app-targets";

export function getDesktopPromptAttentionMeta(prompt: {
  title: string;
  detail: string;
  page: AppTarget["page"];
  target?: AppTarget | null;
}): {
  title: string;
  description: string;
  action: string;
} {
  const action = getOpenTargetLabel(
    prompt.target ?? {
      page: prompt.page,
    },
  );

  if (prompt.page === "search-console") {
    return {
      title: prompt.title,
      description:
        prompt.detail || "Search-related files changed. Re-check SEO and indexing from here.",
      action,
    };
  }
  if (prompt.page === "updates") {
    return {
      title: prompt.title,
      description:
        prompt.detail ||
        "Dependency files changed. Re-check updates and vulnerabilities from here.",
      action,
    };
  }
  return {
    title: prompt.title,
    description: prompt.detail || "Launch-sensitive files changed. Re-open Issues from here.",
    action,
  };
}
