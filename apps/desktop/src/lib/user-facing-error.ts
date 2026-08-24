import { errorMessage } from "@/lib/error-message";

const MAX_MESSAGE_LENGTH = 240;
// Rust `Result<T, String>` rejections and transport wrappers add prefixes
// that carry no meaning for a person.
const TECHNICAL_PREFIX = /^(?:error|invoke error|tauri error|command error)\s*:\s*/i;
const WORDLESS = new Set(["", "[object Object]", "null", "undefined"]);

/** Turn an unknown rejection into one sentence a non-engineer can act on. */
export function userFacingError(error: unknown, fallback: string): string {
  // Tauri can wrap a rejection twice ("Error: invoke error: no such project"),
  // so strip until nothing transport-shaped is left rather than once.
  let raw = errorMessage(error).trim();
  while (TECHNICAL_PREFIX.test(raw)) raw = raw.replace(TECHNICAL_PREFIX, "").trim();
  if (WORDLESS.has(raw)) return fallback;
  const capitalized = raw.charAt(0).toUpperCase() + raw.slice(1);
  const sentence = /[.!?]$/.test(capitalized) ? capitalized : `${capitalized}.`;
  if (sentence.length <= MAX_MESSAGE_LENGTH) return sentence;
  return `${sentence.slice(0, MAX_MESSAGE_LENGTH - 3).trimEnd()}...`;
}
