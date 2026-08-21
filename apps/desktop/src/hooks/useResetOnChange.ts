import { useState } from "react";

/** Reset pure component state before paint when a key changes. */
export function useResetOnChange<T>(key: T, reset: () => void): void {
  const [prevKey, setPrevKey] = useState(key);
  if (!Object.is(key, prevKey)) {
    setPrevKey(key);
    reset();
  }
}
