const RELEASE_WORKFLOW = ".github/workflows/release.yml";
const STANDALONE_WORKFLOW = ".github/workflows/publish-capability-manifest.yml";
const PUBLISHER = "tools/scripts/publish-capability-manifest.mjs";
const MANIFEST_ARTIFACT = "apps/desktop/src-tauri/crates/engine/manifest/capability_manifest.json";

const MANIFEST_TEST = "apps/desktop/src-tauri/crates/engine/tests/capability_manifest.rs";

const REGENERATE_COMMAND = "--ignored regenerate";

const PUBLISH_JOB = "publish-capability-manifest";
const BUILD_JOB = "build";

const CONNECT_ORIGIN = "https://connect.sitecmd.com";
const MANIFEST_ROUTE = "/v1/engine-manifests/";

/** Parse top-level workflow jobs as name-to-body entries. */
function jobsOf(source) {
  const jobs = new Map();
  const start = source.search(/^jobs:\s*$/m);
  if (start === -1) return jobs;
  const region = source.slice(start);
  const headers = [...region.matchAll(/^ {2}([A-Za-z0-9_-]+):\s*$/gm)];
  for (const [index, header] of headers.entries()) {
    const end = index + 1 < headers.length ? headers[index + 1].index : region.length;
    jobs.set(header[1], region.slice(header.index, end));
  }
  return jobs;
}

/** Parse bare, inline-list, or wrapped-list `needs` declarations. */
function needsOf(body) {
  const lines = body.split("\n");
  const declared = lines.findIndex((line) => /^ {4}needs:/.test(line));
  if (declared === -1) return [];
  const declaration = [lines[declared].slice(lines[declared].indexOf(":") + 1)];
  // Stop at the next job-level key, not a deeper wrapped value.
  for (let line = declared + 1; line < lines.length && /^ {5,}\S/.test(lines[line]); line += 1) {
    declaration.push(lines[line]);
  }
  const named = declaration.join("\n").replace(/#.*$/gm, "");
  return [...named.matchAll(/[A-Za-z0-9_-]+/g)].map((token) => token[0]);
}

function transitiveNeeds(jobs, start) {
  const reached = new Set();
  const pending = [...needsOf(jobs.get(start) ?? "")];
  while (pending.length > 0) {
    const job = pending.pop();
    if (reached.has(job)) continue;
    reached.add(job);
    pending.push(...needsOf(jobs.get(job) ?? ""));
  }
  return reached;
}

/** One workflow-level mapping block, excluding every later top-level key. */
function topLevelBlock(source, key) {
  const lines = source.split("\n");
  const start = lines.findIndex((line) => line.trimEnd() === `${key}:`);
  if (start === -1) return "";
  const block = [];
  for (const line of lines.slice(start + 1)) {
    // Column-zero comments do not end the workflow block.
    if (!/^[ \t]/.test(line) && !/^[ \t]*#/.test(line)) break;
    block.push(line);
  }
  return block.length === 0 ? "" : `${block.join("\n")}\n`;
}

/** Workflow permissions inherited by jobs without their own block. */
function topLevelPermissions(source) {
  return topLevelBlock(source, "permissions");
}

/**
 * @param {(file: string) => string} read
 * @param {(dir: string, predicate: (file: string) => boolean) => string[]} listFiles
 */
export function manifestPublicationFailures(read, listFiles) {
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };

  const release = read(RELEASE_WORKFLOW);
  const releaseJobs = jobsOf(release);
  check(
    releaseJobs.has(PUBLISH_JOB),
    `${RELEASE_WORKFLOW} must keep a ${PUBLISH_JOB} job. The manifest has to reach the registry before any build ships under its digest; without this job a release produces artifacts whose observations connect can only quarantine as incomparable.`,
  );
  check(
    transitiveNeeds(releaseJobs, BUILD_JOB).has(PUBLISH_JOB),
    `${RELEASE_WORKFLOW}: the ${BUILD_JOB} job must be transitively unreachable without ${PUBLISH_JOB}. A publication job nothing waits on is decorative - its failure would leave the build running and the release shipping under an unregistered digest.`,
  );

  const workflows = listFiles(".github/workflows", (file) => /\.ya?ml$/.test(file));
  const publishing = workflows.filter((file) => read(file).includes(PUBLISHER));
  check(
    publishing.length > 0,
    `No workflow runs ${PUBLISHER}. A publisher nothing calls is the same as no publisher: the first site to connect loses its verification path silently.`,
  );
  for (const file of publishing) {
    const source = read(file);
    const inherited = topLevelPermissions(source);
    for (const [name, body] of jobsOf(source)) {
      if (!body.includes(PUBLISHER)) continue;
      check(
        /id-token:\s*write/.test(body) || /id-token:\s*write/.test(inherited),
        `${file} job ${name} runs the publisher without id-token: write. The Actions OIDC token is the whole credential the registry accepts, so the job would mint nothing and the publication would fail for a permissions reason that reads as a wiring bug.`,
      );
    }
  }

  const publisher = read(PUBLISHER);
  check(
    publisher.includes(`"${CONNECT_ORIGIN}"`),
    `${PUBLISHER} must address ${CONNECT_ORIGIN}. A publisher pointed somewhere else fails the way a publisher that never ran fails: the build ships under a digest the registry never learned.`,
  );
  check(
    publisher.includes(`"${MANIFEST_ROUTE}"`),
    `${PUBLISHER} must PUT to ${MANIFEST_ROUTE}<digest>, the route the connect Worker serves. Another path answers 404, and a 404 here is an unconfigured door rather than nothing to do.`,
  );
  check(
    /OIDC_AUDIENCE\s*=\s*(?:CONNECT_ORIGIN|"https:\/\/connect\.sitecmd\.com")/.test(publisher),
    `${PUBLISHER} must mint its OIDC token for the ${CONNECT_ORIGIN} audience. A token minted for another audience is a token for another door, and the registry refuses it.`,
  );

  check(
    publisher.includes(REGENERATE_COMMAND),
    `${PUBLISHER} must hand back the command that regenerates the artifact ("${REGENERATE_COMMAND}"). The unignored test only asserts the manifest is current, so naming it sends an operator to reproduce the failure instead of fixing it.`,
  );
  check(
    /#\[ignore[^\]]*\]\s*fn regenerate\(\)/.test(read(MANIFEST_TEST)),
    `${MANIFEST_TEST} must keep the ignored \`regenerate\` test. It is the command ${PUBLISHER} and the release runbook tell an operator to run, and an instruction naming a test that no longer exists is worse than no instruction.`,
  );

  const standalone = read(STANDALONE_WORKFLOW);
  check(
    standalone.includes(MANIFEST_ARTIFACT),
    `${STANDALONE_WORKFLOW} must watch ${MANIFEST_ARTIFACT} in its push paths. It is what keeps the registry ahead of main: a regenerated manifest that triggers nothing stays unpublished until a release notices.`,
  );
  check(
    !/^\s*concurrency:/m.test(standalone),
    `${STANDALONE_WORKFLOW} must not use concurrency grouping. GitHub keeps at most one pending run per concurrency group and replaces an older pending run, so grouping these content-addressed publications can discard an intermediate digest that a developer build still uses.`,
  );

  return failures;
}
