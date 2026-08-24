import type { LanguageFn } from "highlight.js";

export type HighlightLanguages = Record<string, LanguageFn>;

// Each grammar is its own chunk; only the ones a document fences are fetched.
const GRAMMAR_LOADERS: Record<string, () => Promise<{ default: LanguageFn }>> = {
  bash: () => import("highlight.js/lib/languages/bash"),
  css: () => import("highlight.js/lib/languages/css"),
  javascript: () => import("highlight.js/lib/languages/javascript"),
  json: () => import("highlight.js/lib/languages/json"),
  markdown: () => import("highlight.js/lib/languages/markdown"),
  python: () => import("highlight.js/lib/languages/python"),
  rust: () => import("highlight.js/lib/languages/rust"),
  scss: () => import("highlight.js/lib/languages/scss"),
  shell: () => import("highlight.js/lib/languages/shell"),
  typescript: () => import("highlight.js/lib/languages/typescript"),
  // `xml` covers HTML / XML / SVG in highlight.js.
  xml: () => import("highlight.js/lib/languages/xml"),
  yaml: () => import("highlight.js/lib/languages/yaml"),
};

export const ALIASES: Record<string, string[]> = {
  javascript: ["js", "jsx"],
  typescript: ["ts", "tsx"],
  shell: ["sh"],
  xml: ["html"],
};

const ALIAS_TO_LANGUAGE = new Map<string, string>(
  Object.entries(ALIASES).flatMap(([language, aliases]) =>
    aliases.map((alias) => [alias, language] as const),
  ),
);

const loaded: HighlightLanguages = {};
const pending = new Map<string, Promise<void>>();

/** Canonical grammar names fenced in a document; unknown languages are dropped. */
export function fencedLanguages(source: string): string[] {
  const names = new Set<string>();
  for (const match of source.matchAll(/^```([A-Za-z0-9_+-]+)/gm)) {
    const requested = match[1].toLowerCase();
    const language = ALIAS_TO_LANGUAGE.get(requested) ?? requested;
    if (language in GRAMMAR_LOADERS) names.add(language);
  }
  return [...names];
}

/** Every grammar fetched so far in this session. */
export function loadedHighlightLanguages(): HighlightLanguages {
  return { ...loaded };
}

/** Fetch the named grammars once each; resolves with everything loaded so far. */
export async function loadHighlightLanguages(names: string[]): Promise<HighlightLanguages> {
  await Promise.all(
    names.map((name) => {
      if (loaded[name]) return Promise.resolve();
      let task = pending.get(name);
      if (!task) {
        task = GRAMMAR_LOADERS[name]().then((module) => {
          loaded[name] = module.default;
          pending.delete(name);
        });
        pending.set(name, task);
      }
      return task;
    }),
  );
  return loadedHighlightLanguages();
}
