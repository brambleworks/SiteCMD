import { describe, expect, it } from "vitest";
import {
  queueUnlockCopyFailures,
  visibleCopySpans,
} from "./lib/guardrail-machine-smell-copy-rules.mjs";

function harness(files) {
  return {
    read: (file) => files[file] ?? "",
    exists: (path) => path in files || Object.keys(files).some((key) => key.startsWith(`${path}/`)),
    listFiles: (dir, filter) =>
      Object.keys(files).filter((file) => file.startsWith(`${dir}/`) && (!filter || filter(file))),
  };
}

describe("visibleCopySpans", () => {
  it("keeps multi-word copy from quotes, JSX text, and templates", () => {
    expect(visibleCopySpans('cta: "Unlock Deep Scan"')).toContain("Unlock Deep Scan");
    expect(visibleCopySpans("<h3>Fix Queue</h3>")).toContain("Fix Queue");
    expect(visibleCopySpans("`${n} focus areas queued`").join(" ")).toContain("focus areas queued");
  });

  it("skips identifier strings, storage keys, and CSS class lists", () => {
    expect(visibleCopySpans('const K = "sitecmd_telemetry_queue_v1";')).toEqual([]);
    expect(visibleCopySpans('className="queue-tool-button ml-auto"')).toEqual([]);
    expect(
      visibleCopySpans("`Unsent events: ${readQueuedUsageEvents().length}`").join(" "),
    ).not.toContain("readQueuedUsageEvents");
  });
});

describe("queueUnlockCopyFailures", () => {
  it("flags 'unlock' and 'queue' in shipped UI copy", () => {
    const h = harness({
      "apps/desktop/src/a.tsx": 'const x = "Unlock Deep Scan";',
      "apps/desktop/src/b.tsx": 'const y = "The work queue: issues";',
    });
    const failures = queueUnlockCopyFailures(h.read, h.exists, h.listFiles);
    expect(failures.some((f) => /a\.tsx.*unlock/i.test(f))).toBe(true);
    expect(failures.some((f) => /b\.tsx.*queue/i.test(f))).toBe(true);
  });

  it("exempts fix-guide queue vocabulary, allow-marker, dead FixQueue, and tests", () => {
    const h = harness({
      "apps/desktop/src/lib/code-fix-guides/x.ts":
        'const g = "use a shared queue for rate limiting";',
      "apps/desktop/src/c.tsx": 'const z = "the work queue"; // allow-machine-smell',
      "apps/desktop/src/components/dashboard/FixQueueLegacyMode.tsx": 'const d = "Fix Queue";',
      "apps/desktop/src/d.test.tsx": 'expect("the work queue").toBeTruthy();',
    });
    expect(queueUnlockCopyFailures(h.read, h.exists, h.listFiles)).toEqual([]);
  });
});
