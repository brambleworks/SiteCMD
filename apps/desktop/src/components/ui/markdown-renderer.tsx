import { useEffect, useMemo, useState, type ComponentProps } from "react";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import {
  ALIASES,
  fencedLanguages,
  loadHighlightLanguages,
  loadedHighlightLanguages,
  type HighlightLanguages,
} from "./markdown-languages";

type RehypePlugins = NonNullable<ComponentProps<typeof ReactMarkdown>["rehypePlugins"]>;

// Permit syntax-highlighting classes while retaining the default sanitization schema.
const SANITIZE_SCHEMA = {
  ...defaultSchema,
  attributes: {
    ...defaultSchema.attributes,
    code: [...(defaultSchema.attributes?.code ?? []), ["className", /^language-./]],
    span: [...(defaultSchema.attributes?.span ?? []), ["className", /^hljs-/]],
  },
};

interface MarkdownRendererProps {
  children: string;
  className?: string;
}

/** Loaded through `Markdown`; registers only the grammars the document fences. */
export default function MarkdownRenderer({ children, className = "" }: MarkdownRendererProps) {
  const needed = useMemo(() => fencedLanguages(children), [children]);
  const [languages, setLanguages] = useState<HighlightLanguages>(loadedHighlightLanguages);

  useEffect(() => {
    let cancelled = false;
    void loadHighlightLanguages(needed).then((next) => {
      if (!cancelled) setLanguages(next);
    });
    return () => {
      cancelled = true;
    };
  }, [needed]);

  const rehypePlugins = useMemo<RehypePlugins>(
    () => [
      [rehypeHighlight, { languages, aliases: ALIASES }],
      // Sanitize after highlighting to preserve generated syntax classes.
      [rehypeSanitize, SANITIZE_SCHEMA],
    ],
    [languages],
  );

  return (
    <div className={`markdown-body ${className}`}>
      <ReactMarkdown rehypePlugins={rehypePlugins}>{children}</ReactMarkdown>
    </div>
  );
}
