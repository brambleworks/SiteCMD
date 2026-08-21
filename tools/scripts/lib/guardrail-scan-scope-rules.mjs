const ENGINE_SCOPE = "apps/desktop/src-tauri/crates/engine/src/scope.rs";
const ENGINE_ROUTE = "apps/desktop/src-tauri/crates/engine/src/route.rs";
const SCHEDULER = "apps/desktop/src-tauri/src/background/scan_scheduler.rs";
const SCOPE_COMMANDS = "apps/desktop/src-tauri/src/commands/scan_scope.rs";
const OVERLAY_STATE = "apps/desktop/src/components/scan/useScanConfigOverlayState.ts";

export function scanScopeFailures(read) {
  const failures = [];

  const engine = read(ENGINE_SCOPE);
  for (const symbol of [
    "pub const SCOPE_WIRE_LIMIT",
    "pub const HOSTED_SCOPE_CEILING",
    "pub fn build_scope",
  ]) {
    if (!engine.includes(symbol)) {
      failures.push(
        `${ENGINE_SCOPE} must keep ${symbol}: the scope's bounds and its construction are what a connected PUT is judged by, and a desktop copy would be a second answer.`,
      );
    }
  }
  if (!engine.includes("entry_route")) {
    failures.push(
      `${ENGINE_SCOPE} must keep the entry route in the scope it builds; origin-scoped checks run on the entry page, so a scope without it could never cover them.`,
    );
  }

  const route = read(ENGINE_ROUTE);
  if (!/pub const CANONICALIZER_VERSION/.test(route)) {
    failures.push(
      `${ENGINE_ROUTE} must keep CANONICALIZER_VERSION: a change to the route rules is a new version through the compatibility gate, not an edit to the old one.`,
    );
  }

  const scheduler = read(SCHEDULER);
  if (!scheduler.includes("scan_scope_urls")) {
    failures.push(
      `${SCHEDULER} must resolve its URLs through scan_scope_urls. A scheduled run that scans the entry URL alone watches less than the owner selected, and it is the run nobody is present to notice.`,
    );
  }

  const commands = read(SCOPE_COMMANDS);
  if (!commands.includes("build_scope")) {
    failures.push(
      `${SCOPE_COMMANDS} must build its scope through sitecmd_engine::scope::build_scope so canonicalization, entry-route inclusion, and the bounds are decided in one place.`,
    );
  }
  for (const pattern of [/routes\.truncate\(/, /routes\.iter\(\)\.take\(/]) {
    if (pattern.test(commands)) {
      failures.push(
        `${SCOPE_COMMANDS} must never truncate a scope to fit. An over-limit scope is refused with the limit named; silently trimming it leaves routes listed as watched that nothing will ever scan.`,
      );
    }
  }

  const overlay = read(OVERLAY_STATE);
  if (!overlay.includes("setScanScope")) {
    failures.push(
      `${OVERLAY_STATE} must record the selection as the site's scan scope; the page checklist IS the authoring surface, and a selection that is not stored is one the schedule never sees.`,
    );
  } else {
    const start = overlay.indexOf("const handleStart");
    const body = overlay.slice(start, overlay.indexOf("\n  }, [", start));
    // The final dispatch is the scoped web scan; code-only scans have no routes.
    if (body.indexOf("await setScanScope") > body.lastIndexOf("onStart(")) {
      failures.push(
        `${OVERLAY_STATE}: handleStart must await the scope write BEFORE dispatching the run, so a refused write stops the run instead of leaving this scan and the next scheduled one covering different routes.`,
      );
    }
  }

  return failures;
}
