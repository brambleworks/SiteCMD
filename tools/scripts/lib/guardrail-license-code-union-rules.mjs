import { LIFECYCLE_DEACTIVATION } from "./guardrail-license-sources.mjs";
import { stripComments, stripNonCode } from "./guardrail-source-text.mjs";

const RUST = "apps/desktop/src-tauri/src/licensing/activation_errors.rs";
const TS = "apps/desktop/src/lib/license-activation-error.ts";

function snakeCase(variant) {
  return variant.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
}

export function licenseCodeUnionFailures(read) {
  const failures = [];
  const rust = stripComments(read(RUST), RUST);
  const ts = stripComments(read(TS), TS);

  const enumStart = rust.indexOf("pub enum LicenseActivationErrorCode {");
  const enumEnd = enumStart === -1 ? -1 : rust.indexOf("\n}", enumStart);
  if (enumStart === -1 || enumEnd === -1) {
    failures.push(`${RUST} must declare pub enum LicenseActivationErrorCode`);
    return failures;
  }
  const enumBody = rust.slice(enumStart, enumEnd);
  const rustCodes = new Set(
    [...enumBody.matchAll(/^\s{4}([A-Z][A-Za-z0-9]*),$/gm)].map((match) => snakeCase(match[1])),
  );
  const declared = [...enumBody.matchAll(/^\s{4}([A-Z][A-Za-z0-9]*)/gm)].map((match) => match[1]);
  if (declared.length !== rustCodes.size) {
    failures.push(
      `${RUST}: LicenseActivationErrorCode declares ${declared.length} variants but only ${rustCodes.size} are in the shape this check can read (bare, comma-terminated). The union comparison below would silently ignore the difference.`,
    );
    return failures;
  }

  const setStart = ts.indexOf("KNOWN_CODES");
  const setEnd = setStart === -1 ? -1 : ts.indexOf("]);", setStart);
  if (setStart === -1 || setEnd === -1) {
    failures.push(`${TS} must declare the KNOWN_CODES set`);
    return failures;
  }
  const tsCodes = new Set(
    [...ts.slice(setStart, setEnd).matchAll(/"([a-z0-9_]+)"/g)].map((match) => match[1]),
  );

  if (rustCodes.size === 0 || tsCodes.size === 0) {
    failures.push(
      `${RUST} / ${TS}: could not read the activation error codes from either side; the union check is not actually running`,
    );
    return failures;
  }

  const missingInTs = [...rustCodes].filter((code) => !tsCodes.has(code)).sort();
  const missingInRust = [...tsCodes].filter((code) => !rustCodes.has(code)).sort();
  if (missingInTs.length > 0) {
    failures.push(
      `${TS} is missing activation error codes the Rust side can emit (${missingInTs.join(", ")}); the frontend maps an unrecognized code to "unknown", which reports a conclusive refusal as an unfinished attempt`,
    );
  }
  if (missingInRust.length > 0) {
    failures.push(
      `${TS} declares activation error codes the Rust side never emits (${missingInRust.join(", ")}); one of the two lists is stale`,
    );
  }
  failures.push(...deactivationMarkerFailures(read));
  return failures;
}

const RESULT_CALLS = new Set([
  "format",
  "released_clause",
  "remaining_work",
  "leaves_a_stranded_seat",
]);

const DEACTIVATION_RUST = LIFECYCLE_DEACTIVATION;
const DEACTIVATION_TS = "apps/desktop/src/lib/license-deactivation.ts";
const DEACTIVATION_PANEL = "apps/desktop/src/components/settings/AccountSettings.tsx";

function deactivationMarkerFailures(read) {
  const failures = [];
  const marker = /"(unlinked_with_keychain_remnant: )"/;
  const rust = stripComments(read(DEACTIVATION_RUST), DEACTIVATION_RUST);
  const ts = stripComments(read(DEACTIVATION_TS), DEACTIVATION_TS);
  const panel = stripComments(read(DEACTIVATION_PANEL), DEACTIVATION_PANEL);
  if (!marker.test(rust) || !marker.test(ts)) {
    return [
      `${DEACTIVATION_RUST} and ${DEACTIVATION_TS} must both declare the "unlinked_with_keychain_remnant: " marker; without it a completed unlink is reported to the user as a failed one`,
    ];
  }
  const resultStart = rust.indexOf("pub(super) fn deactivation_result");
  const resultEnd = resultStart === -1 ? -1 : rust.indexOf("\n}", resultStart);
  if (resultStart === -1 || resultEnd === -1) {
    failures.push(
      `${DEACTIVATION_RUST} must define deactivation_result; it is the single place a completed unlink is turned into an Err, and the marker rule is scoped to it`,
    );
    return failures;
  }
  const resultBody = rust.slice(resultStart, resultEnd);
  const errs = (resultBody.match(/Err\(/g) ?? []).length;
  const marked = (resultBody.match(/Err\(format!\(\s*"\{DEACTIVATION_KEYCHAIN_REMNANT\}/g) ?? [])
    .length;
  if (errs === 0 || errs !== marked) {
    failures.push(
      `${DEACTIVATION_RUST}: deactivation_result returns ${errs} Err(...), of which ${marked} are an Err(format!(...)) OPENING with DEACTIVATION_KEYCHAIN_REMNANT. Every error it mints reports a completed unlink with something left over, and the frontend recognizes that by a startsWith on the prefix - one that is unmarked, or marked anywhere but the front, is shown to the user under "Deactivation failed" above a sentence saying the machine was unlinked`,
    );
  }
  const code = stripNonCode(read(DEACTIVATION_RUST), DEACTIVATION_RUST);
  const bodyStart = code.indexOf("{", resultStart);
  const delegated = [...code.slice(bodyStart, resultEnd).matchAll(/\b([a-z_][a-z0-9_]*)!?\s*\(/g)]
    .map((match) => match[1])
    .filter((name) => !RESULT_CALLS.has(name));
  if (delegated.length > 0) {
    failures.push(
      `${DEACTIVATION_RUST}: deactivation_result calls ${[...new Set(delegated)].join(", ")}, which the marker rule cannot see into. It must build every error it returns itself, from ${[...RESULT_CALLS].join(", ")} - an arm that delegates mints an unmarked completed-unlink error while the arms left behind keep the counts equal`,
    );
  }
  if (!/startsWith\(DEACTIVATION_KEYCHAIN_REMNANT\)/.test(panel)) {
    failures.push(
      `${DEACTIVATION_PANEL} must branch on DEACTIVATION_KEYCHAIN_REMNANT with startsWith; without that branch the marker is inert and a completed unlink is reported under "Deactivation failed"`,
    );
  }
  // Strip the internal prefix before display.
  if (!/slice\(DEACTIVATION_KEYCHAIN_REMNANT\.length\)/.test(panel)) {
    failures.push(
      `${DEACTIVATION_PANEL} must strip DEACTIVATION_KEYCHAIN_REMNANT off the message before showing it, or the marker is displayed to the user`,
    );
  }
  return failures;
}
