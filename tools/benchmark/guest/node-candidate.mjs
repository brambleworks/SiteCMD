import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

const input = JSON.parse(readFileSync(0, "utf8"));
try {
  if (input.operation === "public-tests") {
    const result = spawnSync(process.execPath, ["--test"], {
      encoding: "utf8",
      timeout: 5000,
      maxBuffer: 512 * 1024,
    });
    process.stdout.write(
      JSON.stringify({ exitCode: result.status, log: `${result.stdout}${result.stderr}` }),
    );
    process.exit(result.error ? 1 : 0);
  }
  const candidate = await import(pathToFileURL(`/work/${input.entry}`).href);
  const response = await candidate.GET(new Request(input.url, { headers: input.headers }));
  process.stdout.write(
    JSON.stringify({
      status: response.status,
      headers: Object.fromEntries(response.headers),
      body: await response.text(),
    }),
  );
} catch (error) {
  process.stdout.write(JSON.stringify({ error: error.name, message: error.message }));
}
