import { useSyncExternalStore } from "react";
import { getScanRunGlidePercent, subscribeScanRunGlide } from "@/lib/scan-run-glide";

/**
 * The active step's displayed percent as a smooth, fractional value that
 * changes on every tick of the glide clock. For the ring and the bar only;
 * anything larger should follow `useScanRunWholePercent` so it repaints on
 * whole numbers instead of twenty times a second.
 */
export function useScanRunPercent(): number {
  return useSyncExternalStore(subscribeScanRunGlide, getScanRunGlidePercent);
}
