import { invoke } from "@/lib/tauri-invoke";

export function command<T>(name: string, args?: object): Promise<T> {
  // Omit the argument entirely for no-argument commands.
  return args === undefined ? invoke<T>(name) : invoke<T>(name, args as Record<string, unknown>);
}
