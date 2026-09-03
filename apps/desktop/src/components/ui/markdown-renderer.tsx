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
import { ExtLink } from "./external-link";

type RehypePlugins = NonNullable<ComponentProps<typeof ReactMarkdown>["rehypePlugins"]>;
type MarkdownComponents = NonNullable<ComponentProps<typeof ReactMarkdown>["components"]>;

// Markdown here can carry scanned-site or repository text (issue evidence,
// fix prompts), so a bare `<a href>` would let a page title navigate the app
// window and a bare `<img>` would beacon to any origin the moment the issue
// is expanded. Links go through the same confirmed opener as every other
// external link; images are dropped.
const MARKDOWN_COMPONENTS: MarkdownComponents = {
  a: ({ href, children }) =>
    href ? <ExtLink href={href}>{children}</ExtLink> : <span>{children}</span>,
  img: () => null,
};

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
      <ReactMarkdown rehypePlugins={rehypePlugins} components={MARKDOWN_COMPONENTS}>
        {children}
      </ReactMarkdown>
    </div>
  );
}
