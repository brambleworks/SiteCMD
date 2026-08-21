const CONFIG = "apps/desktop/src-tauri/tauri.conf.json";
const RUNBOOK = "docs/engineering/release-signing-key-rotation.md";

// Minisign's public-key identifier header.
const KEY_ID = /minisign public key:\s*([0-9A-F]{16})/i;

export function updaterTrustFailures(read, exists) {
  if (!exists(CONFIG)) return [];

  let pubkey;
  try {
    pubkey = JSON.parse(read(CONFIG))?.plugins?.updater?.pubkey;
  } catch (error) {
    return [`${CONFIG} is not valid JSON (${error.message}).`];
  }

  if (typeof pubkey !== "string" || pubkey.length === 0) {
    return [
      `${CONFIG} has no updater pubkey. A build shipped without one cannot verify any update, and the updater is the only way a released client ever changes.`,
    ];
  }

  let decoded;
  try {
    decoded = Buffer.from(pubkey, "base64").toString("utf8");
  } catch {
    return [`${CONFIG}: plugins.updater.pubkey is not decodable base64.`];
  }

  const id = KEY_ID.exec(decoded)?.[1]?.toUpperCase();
  if (!id) {
    return [
      `${CONFIG}: plugins.updater.pubkey does not decode to a minisign public key block, so the key it pins cannot be identified.`,
    ];
  }

  if (!exists(RUNBOOK)) {
    return [
      `${RUNBOOK} is missing, and it is the only record of which updater signing keys exist and which releases carry them.`,
    ];
  }

  const runbook = read(RUNBOOK);
  const unsupportedTransitions = ["updater.pubkeyNext", "updater-trust.json"].filter((term) =>
    runbook.includes(term),
  );
  if (unsupportedTransitions.length > 0) {
    return [
      `${RUNBOOK} describes an unsupported updater transition (${unsupportedTransitions.join(", ")}). The current app trusts one embedded key, so its incident procedure must require a separately authenticated fresh installer rather than fictional dynamic trust fields.`,
    ];
  }

  if (!runbook.toUpperCase().includes(id)) {
    return [
      `${CONFIG} pins updater key ${id} and ${RUNBOOK} does not mention it. Rotating this key decides which installed builds can ever update again: every build carrying a different key refuses everything signed with this one, permanently, and no release check can see that because each one verifies a release against its own commit's key. Record the rotation in the runbook's audit history - which key, when, which released tag last carried the previous one, and whether a transitional release was cut - before shipping it.`,
    ];
  }

  return [];
}
