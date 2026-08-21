const SETUP = "apps/desktop/src-tauri/src/commands/connected_setup.rs";
const CLIENT = "apps/desktop/src-tauri/src/connected_service.rs";

/** Maximum distance between a failure return and its cleanup call. */
const CLEANUP_WINDOW_LINES = 3;

export function connectedSetupFailures(read) {
  const failures = [];

  const client = read(CLIENT);
  if (!client.includes("pub async fn delete_site")) {
    failures.push(
      `${CLIENT}: ConnectedServiceClient no longer offers delete_site; ` +
        `setup rollback needs it to clean up the remote site it created`,
    );
  }

  const source = read(SETUP);
  const fnStart = source.indexOf("pub async fn create_connected_site");
  if (fnStart === -1) {
    failures.push(`${SETUP} no longer defines create_connected_site; update these rules with it`);
    return failures;
  }
  const nextFn = source.indexOf("pub async fn", fnStart + 1);
  const body = source.slice(fnStart, nextFn === -1 ? source.length : nextFn);
  const lines = body.split("\n");
  const createLine = lines.findIndex((line) => line.includes(".create_site("));
  if (createLine === -1) {
    failures.push(
      `${SETUP}: create_connected_site no longer calls create_site; update these rules with it`,
    );
    return failures;
  }

  for (let index = createLine + 1; index < lines.length; index += 1) {
    if (!lines[index].includes("return Err(")) continue;
    const above = lines.slice(Math.max(createLine, index - CLEANUP_WINDOW_LINES), index).join("\n");
    if (!above.includes("delete_site")) {
      failures.push(
        `${SETUP}: the failure return on line ${index + 1} of create_connected_site ` +
          `does not delete the remote site first; a failure after the remote create ` +
          `must clean up best-effort or the site is orphaned`,
      );
    }
  }

  return failures;
}
