interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export function readCurrentOrLegacyValue(
  storage: StorageLike,
  currentKey: string,
  legacyKey: string,
): string | null {
  return storage.getItem(currentKey) ?? storage.getItem(legacyKey);
}

export function migrateLegacyValue(
  storage: StorageLike,
  currentKey: string,
  legacyKey: string,
): string | null {
  const currentValue = storage.getItem(currentKey);
  if (currentValue != null) return currentValue;

  const legacyValue = storage.getItem(legacyKey);
  if (legacyValue == null) return null;

  storage.setItem(currentKey, legacyValue);
  storage.removeItem(legacyKey);
  return legacyValue;
}

export function writeCurrentValue(
  storage: StorageLike,
  currentKey: string,
  legacyKey: string,
  value: string,
): void {
  storage.setItem(currentKey, value);
  storage.removeItem(legacyKey);
}
