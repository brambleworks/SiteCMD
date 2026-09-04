import { command } from "./invoke";

/** Read a key from the fixed application settings store. */
export function getAppSetting<T>(key: string): Promise<T | null> {
  return command<T | null>("get_app_setting", { key });
}

/** Update a key in the fixed application settings store. */
export function setAppSetting<T>(key: string, value: T): Promise<void> {
  return command<void>("set_app_setting", { key, value });
}
