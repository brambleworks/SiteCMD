import type { ComponentProps } from "react";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import scss from "highlight.js/lib/languages/scss";
import shell from "highlight.js/lib/languages/shell";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

const LANGUAGES = {
  bash,
  css,
  javascript,
  json,
  markdown,
  python,
  rust,
  scss,
  shell,
  typescript,
  // `xml` covers HTML / XML / SVG in highlight.js.
  xml,
  yaml,
};

const ALIASES = {
  javascript: ["js", "jsx"],
  typescript: ["ts", "tsx"],
  shell: ["sh"],
  xml: ["html"],
};

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

const REHYPE_PLUGINS: RehypePlugins = [
  [rehypeHighlight, { languages: LANGUAGES, aliases: ALIASES }],
  // Sanitize after highlighting to preserve generated syntax classes.
  [rehypeSanitize, SANITIZE_SCHEMA],
];

interface MarkdownProps {
  children: string;
  className?: string;
}

export function Markdown({ children, className = "" }: MarkdownProps) {
  return (
    <div className={`markdown-body ${className}`}>
      <ReactMarkdown rehypePlugins={REHYPE_PLUGINS}>{children}</ReactMarkdown>
    </div>
  );
}
