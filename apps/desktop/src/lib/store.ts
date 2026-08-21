import { LazyStore } from "@tauri-apps/plugin-store";

const store = new LazyStore("settings.json", { defaults: {}, autoSave: true });

/** Read a value from the Tauri store. Returns `fallback` if not found. */
export async function storeGet<T>(key: string, fallback: T): Promise<T> {
  try {
    const value = await store.get<T>(key);
    return value ?? fallback;
  } catch {
    return fallback;
  }
}

/** Write a value to the durable Tauri store. */
export async function storeSet<T>(key: string, value: T): Promise<void> {
  try {
    await store.set(key, value);
  } catch {
    // localStorage remains the fallback.
  }
}

/** Move a localStorage value into the Tauri store without replacing stored data. */
export async function migrateFromLocalStorage<T>(
  localStorageKey: string,
  storeKey: string,
  fallback: T,
  parseStoredValue: (value: unknown) => T | null,
): Promise<T> {
  const writeLocalStorageFallback = (value: T) => {
    if (typeof window === "undefined") return;
    try {
      window.localStorage.setItem(localStorageKey, JSON.stringify(value));
    } catch {
      // The durable store already has the value.
    }
  };

  try {
    const existing = await store.get<unknown>(storeKey);
    const parsedExisting = parseStoredValue(existing);
    if (parsedExisting != null) {
      writeLocalStorageFallback(parsedExisting);
      return parsedExisting;
    }

    if (typeof window === "undefined") return fallback;

    const raw = window.localStorage.getItem(localStorageKey);
    if (raw) {
      const parsed = parseStoredValue(JSON.parse(raw) as unknown);
      if (parsed == null) return fallback;
      await store.set(storeKey, parsed);
      return parsed;
    }
  } catch {
    // Return the supplied fallback.
  }
  return fallback;
}
