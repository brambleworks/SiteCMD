/** Normalize an unknown rejection value to a displayable message. */
export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error ?? "");
}
