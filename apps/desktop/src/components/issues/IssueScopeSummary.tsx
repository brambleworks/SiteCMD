import { cn } from "@/lib/utils";
import type { IssueScopeMeta } from "@/lib/issue-scope";

const SCOPE_TEXT: Record<IssueScopeMeta["scope"], string> = {
  page: "text-primary",
  site: "text-muted-foreground",
  code: "text-cat-code",
};

export function IssueScopeInline({
  meta,
  className,
  detail,
}: {
  meta: IssueScopeMeta;
  className?: string;
  detail?: string | null;
}) {
  const parts = [meta.scopeLabel, meta.subjectLabel, detail].filter(Boolean);

  return (
    <p className={cn("subtitle-xs text-truncate", className)}>
      <span className={SCOPE_TEXT[meta.scope]}>{meta.scopeLabel}</span>
      {parts.length > 1 && <> · {parts.slice(1).join(" · ")}</>}
    </p>
  );
}
