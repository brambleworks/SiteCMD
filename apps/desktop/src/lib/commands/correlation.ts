import { command } from "./invoke";
import type { Correlation } from "@/generated/ipc-bindings";

export function getCorrelations(args: { projectId: number }): Promise<Correlation[]> {
  return command<Correlation[]>("get_correlations", args);
}
