export type ScanRunMode = "web" | "code" | "full";

export interface ScanRunStep {
  mode: ScanRunMode;
  stepIndex: number;
  stepCount: number;
  label: string;
}
