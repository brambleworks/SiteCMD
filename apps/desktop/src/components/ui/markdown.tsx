import { lazy, Suspense } from "react";

const MarkdownRenderer = lazy(() => import("./markdown-renderer"));

interface MarkdownProps {
  children: string;
  className?: string;
}

/** Markdown with syntax highlighting; the renderer and its grammars load on first use. */
export function Markdown({ children, className = "" }: MarkdownProps) {
  return (
    <Suspense
      fallback={
        <pre className={`markdown-body markdown-body--pending ${className}`}>{children}</pre>
      }>
      <MarkdownRenderer className={className}>{children}</MarkdownRenderer>
    </Suspense>
  );
}
