import { useSyncExternalStore } from "react";
import { getScanRunGlideWholePercent, subscribeScanRunGlide } from "@/lib/scan-run-glide";

/** The displayed percent rounded to a whole number; re-renders only when that number changes. */
export function useScanRunWholePercent(): number {
  return useSyncExternalStore(subscribeScanRunGlide, getScanRunGlideWholePercent);
}
