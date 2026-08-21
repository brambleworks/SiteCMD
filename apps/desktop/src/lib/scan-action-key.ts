let scanActionSequence = 0;

/** A fresh key for one user action. Retries reuse the request object/key. */
export function createScanActionKey(prefix: string): string {
  scanActionSequence += 1;
  const randomId = globalThis.crypto?.randomUUID?.();
  return `${prefix}:${randomId ?? `${Date.now()}-${scanActionSequence}`}`;
}
