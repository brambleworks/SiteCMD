export type JsonRecord = Record<string, unknown>;

export function isJsonRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function parseJsonRecord(raw: string): JsonRecord | null {
  try {
    const parsed: unknown = JSON.parse(raw);
    return isJsonRecord(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export function coerceJsonRecord(value: unknown): JsonRecord | null {
  if (typeof value === "string") return parseJsonRecord(value);
  return isJsonRecord(value) ? value : null;
}

export function parseRecordMap<T>(
  value: unknown,
  parseEntry: (entry: unknown) => T | null,
): Record<string, T> | null {
  if (!isJsonRecord(value)) return null;

  const entries: Array<[string, T]> = [];
  for (const [key, entry] of Object.entries(value)) {
    const parsed = parseEntry(entry);
    if (!parsed) return null;
    entries.push([key, parsed]);
  }
  return Object.fromEntries(entries);
}

export function parseNumberRecord(value: unknown): Record<string, number> | null {
  return parseRecordMap(value, (entry) =>
    typeof entry === "number" && Number.isFinite(entry) ? entry : null,
  );
}
