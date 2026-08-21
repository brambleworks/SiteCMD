import { command } from "./invoke";

export function exportDatabase(args: { destPath: string }): Promise<string> {
  return command<string>("export_database", args);
}

export function importDatabase(args: { srcPath: string }): Promise<string> {
  return command<string>("import_database", args);
}

export function getDbPath(): Promise<string> {
  return command<string>("get_db_path");
}

export function getDbSize(): Promise<number> {
  return command<number>("get_db_size");
}

export function logFrontend(args: {
  level: string;
  message: string;
  context?: string | null;
}): Promise<void> {
  return command<void>("log_frontend", args);
}

export function getLogPath(): Promise<string> {
  return command<string>("get_log_path");
}

export function readRecentLogs(args: { lines?: number }): Promise<string> {
  return command<string>("read_recent_logs", args);
}

export function writeExportFile(args: { path: string; content: string }): Promise<void> {
  return command<void>("write_export_file", args);
}

export function writeExportBytes(args: { path: string; bytes: number[] }): Promise<void> {
  return command<void>("write_export_bytes", args);
}

export function clearScanHistory(): Promise<number> {
  return command<number>("clear_scan_history");
}
