import { isRegexStart, regexEnd } from "./guardrail-source-regex.mjs";

const RUST_CHAR_LITERAL_MAX = 12; // '\u{10FFFF}' is the longest legal one.

const RAW_STRING_START = /[bc]?r(#*)"/y;

function dialectOf(file) {
  if (typeof file !== "string" || file.length === 0) {
    throw new TypeError(
      "stripNonCode/stripComments need the path of the file they are stripping: " +
        "nested block comments and lifetimes make Rust and TypeScript disagree, and a " +
        "stripper that guesses wrong mis-parses one of them without saying so.",
    );
  }
  return /\.rs$/.test(file) ? "rust" : "web";
}

/** Blanks comments and literals while preserving length and newlines. */
export function stripNonCode(source, file) {
  return scan(source, true, dialectOf(file));
}

/** Blanks comments while retaining parsed string literals and source positions. */
export function stripComments(source, file) {
  return scan(source, false, dialectOf(file));
}

function scan(source, blankStrings, dialect) {
  const out = source.split("");
  const ctx = {
    source,
    blankStrings,
    rust: dialect === "rust",
    blank(start, end) {
      for (let i = start; i < end && i < out.length; i += 1) {
        // Preserve line structure and positional anchors.
        if (out[i] !== "\n") out[i] = " ";
      }
    },
  };
  walk(ctx, 0, false);
  return out.join("");
}

function walk(ctx, from, untilCloseBrace) {
  const { source, blankStrings, rust, blank } = ctx;
  let i = from;
  let depth = 0;
  while (i < source.length) {
    const two = source.slice(i, i + 2);
    if (two === "//") {
      const end = source.indexOf("\n", i);
      const stop = end === -1 ? source.length : end;
      blank(i, stop);
      i = stop;
      continue;
    }
    if (two === "/*") {
      // Rust block comments nest; web block comments close at the first marker.
      let j = i + 2;
      let depth = 1;
      while (j < source.length && depth > 0) {
        if (rust && source.startsWith("/*", j)) {
          depth += 1;
          j += 2;
          continue;
        }
        if (source.startsWith("*/", j)) {
          depth -= 1;
          j += 2;
          continue;
        }
        j += 1;
      }
      blank(i, j);
      i = j;
      continue;
    }
    RAW_STRING_START.lastIndex = i;
    const raw = rust ? RAW_STRING_START.exec(source) : null;
    if (raw && !/[A-Za-z0-9_]/.test(source[i - 1] ?? "")) {
      const terminator = `"${raw[1]}`;
      const end = source.indexOf(terminator, i + raw[0].length);
      const stop = end === -1 ? source.length : end + terminator.length;
      if (blankStrings) blank(i, stop);
      i = stop;
      continue;
    }
    if (rust && source[i] === "'" && isLifetimeOrLabel(source, i)) {
      i += 1;
      continue;
    }
    if (!rust && source[i] === "/" && isRegexStart(source, i)) {
      const end = regexEnd(source, i);
      if (end !== -1) {
        if (blankStrings) blank(i, end);
        i = end;
        continue;
      }
    }
    if (!rust && source[i] === "`") {
      i = template(ctx, i);
      continue;
    }
    const quote = source[i];
    if (quote === '"' || quote === "'") {
      let j = i + 1;
      // Bound Rust character literals so a stray apostrophe cannot consume the file.
      const limit =
        rust && quote === "'" ? Math.min(source.length, i + RUST_CHAR_LITERAL_MAX) : source.length;
      let unterminated = false;
      while (j < limit) {
        if (source[j] === "\\") {
          j += 2;
          continue;
        }
        if (!rust && source[j] === "\n") {
          unterminated = true;
          break;
        }
        if (source[j] === quote) break;
        j += 1;
      }
      if (unterminated || j >= limit) {
        i += 1;
        continue;
      }
      const stop = Math.min(j + 1, source.length);
      if (blankStrings) blank(i, stop);
      i = stop;
      continue;
    }
    if (untilCloseBrace) {
      if (source[i] === "{") {
        depth += 1;
      } else if (source[i] === "}") {
        if (depth === 0) return i + 1;
        depth -= 1;
      }
    }
    i += 1;
  }
  return i;
}

function template(ctx, start) {
  const { source, blankStrings, blank } = ctx;
  let i = start + 1;
  let textFrom = start;
  while (i < source.length) {
    if (source[i] === "\\") {
      i += 2;
      continue;
    }
    if (source[i] === "`") {
      if (blankStrings) blank(textFrom, i + 1);
      return i + 1;
    }
    if (source.startsWith("${", i)) {
      if (blankStrings) blank(textFrom, i + 2);
      const holeEnd = walk(ctx, i + 2, true);
      // Blank both interpolation delimiters so the stripped view stays balanced.
      if (blankStrings && source[holeEnd - 1] === "}") blank(holeEnd - 1, holeEnd);
      i = holeEnd;
      textFrom = i;
      continue;
    }
    i += 1;
  }
  if (blankStrings) blank(textFrom, source.length);
  return source.length;
}

function isLifetimeOrLabel(source, index) {
  const match = /^'([A-Za-z_][A-Za-z0-9_]*)/.exec(source.slice(index, index + 64));
  if (!match) return false;
  return source[index + match[0].length] !== "'";
}
