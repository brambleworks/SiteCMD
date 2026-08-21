// A slash opens a regex only where a value may begin.
const REGEX_PRECEDERS = new Set([..."(,=:[!&|?{};+-*%~^<>"]);
const REGEX_KEYWORDS = new Set([
  "return",
  "typeof",
  "instanceof",
  "in",
  "of",
  "new",
  "delete",
  "void",
  "case",
  "do",
  "else",
  "yield",
  "await",
  // `throw` does not end a value and may precede a regex literal.
  "throw",
]);
/** Statement heads whose closing parenthesis may precede a regex statement. */
const STATEMENT_HEADS = new Set(["if", "while", "for"]);

/** Return whether a slash opens a regex based on the preceding syntax. */
export function isRegexStart(source, index) {
  let k = index - 1;
  while (k >= 0 && /\s/.test(source[k])) k -= 1;
  if (k < 0) return true;
  const before = source[k];
  // Postfix increment and decrement end values; binary plus and minus do not.
  if ((before === "+" || before === "-") && source[k - 1] === before) return false;
  // Distinguish postfix non-null assertions from prefix negation.
  if (before === "!") return !endsValue(source, k - 1);
  // A statement-head parenthesis may be followed by a regex statement.
  if (before === ")") return closesStatementHead(source, k);
  if (REGEX_PRECEDERS.has(before)) return true;
  if (!/[A-Za-z0-9_$]/.test(before)) return false;
  const word = /[A-Za-z_$][A-Za-z0-9_$]*$/.exec(source.slice(0, k + 1))?.[0];
  if (word === undefined || !REGEX_KEYWORDS.has(word)) return false;
  // Keywords used as property names end values, so a following slash divides.
  let p = k - word.length;
  while (p >= 0 && /\s/.test(source[p])) p -= 1;
  return source[p] !== ".";
}

/** Whether the significant character at or before `index` ends a value. */
function endsValue(source, index) {
  let i = index;
  while (i >= 0 && /\s/.test(source[i])) i -= 1;
  return i >= 0 && /[A-Za-z0-9_$)\]"'`]/.test(source[i]);
}

/** Whether `)` closes a statement head rather than a call or expression. */
function closesStatementHead(source, index) {
  let depth = 0;
  for (let i = index; i >= 0; i -= 1) {
    if (source[i] === ")") depth += 1;
    else if (source[i] === "(") {
      depth -= 1;
      if (depth === 0) {
        // Treat `for await (...)` as one statement head.
        let head = source.slice(0, i).replace(/\s*\bawait\s*$/, "");
        const word = /[A-Za-z_$][A-Za-z0-9_$]*\s*$/.exec(head)?.[0];
        return word !== undefined && STATEMENT_HEADS.has(word.trim());
      }
    }
  }
  return false;
}

/** Return the end of a same-line regex literal, respecting character classes. */
export function regexEnd(source, start) {
  let i = start + 1;
  let inClass = false;
  while (i < source.length) {
    const ch = source[i];
    if (ch === "\n") return -1;
    if (ch === "\\") {
      i += 2;
      continue;
    }
    if (ch === "[") inClass = true;
    else if (ch === "]") inClass = false;
    else if (ch === "/" && !inClass) {
      // Let the comment parser handle `//` and `/*` openers.
      if (source[i + 1] === "/" || source[i + 1] === "*") return -1;
      i += 1;
      while (i < source.length && /[a-z]/.test(source[i])) i += 1;
      return i;
    }
    i += 1;
  }
  return -1;
}
