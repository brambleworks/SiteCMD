import { readFileSync } from "node:fs";
import path from "node:path";
import { digest } from "./workflow-plan.mjs";

export const calibrationCases = JSON.parse(
  readFileSync(new URL("../cases/calibration.json", import.meta.url), "utf8"),
);

export function caseFiles(item, reference = false) {
  const tests = publicTest(item);
  const files = {
    ...item.files,
    ...(reference ? item.reference : {}),
    ...tests.files,
    "README.md": `# Application contract\n\n${item.requirements}\n\nRun the existing tests with \`${tests.command}\`.\n\nThis is an owned, seeded calibration repository, not a production application.\n`,
  };
  for (const name of Object.keys(files)) {
    if (
      !/^[a-zA-Z0-9_./-]+$/.test(name) ||
      name.split("/").some((part) => !part || part === "." || part === "..")
    )
      throw new Error(`Invalid case file path: ${name}`);
  }
  return files;
}

function publicTest(item) {
  if (item.runtime === "node") {
    const expression =
      item.id === "credentialed-cors"
        ? `const response = GET(new Request('https://api.example.com/account', { headers: { origin: 'https://dashboard.example.com' } }));\nassert.equal(response.status, 200);\nassert.equal(response.headers.get('Access-Control-Allow-Origin'), 'https://dashboard.example.com');`
        : `const response = GET(new Request('https://app.example.com/continue?next=/settings'));\nassert.equal(response.status, 302);\nassert.equal(response.headers.get('location'), 'https://app.example.com/settings');`;
    return {
      command: "node --test",
      files: {
        [path.join(path.dirname(item.entry), "route.test.mjs")]:
          `import assert from 'node:assert/strict';\nimport { GET } from './route.mjs';\n${expression}\n`,
      },
    };
  }
  const body =
    item.id === "document-download"
      ? `        with tempfile.TemporaryDirectory() as root:\n            Path(root, 'readme.txt').write_text('Hello')\n            self.assertEqual(app.download(root, SimpleNamespace(args={'path': 'readme.txt'})), 'Hello')\n`
      : `        connection = sqlite3.connect(':memory:')\n        connection.execute('CREATE TABLE users (id INTEGER, name TEXT)')\n        connection.execute('INSERT INTO users VALUES (?, ?)', (1, 'alice'))\n        self.assertEqual(app.lookup(connection, SimpleNamespace(args={'name': 'alice'})), [(1, 'alice')])\n`;
  return {
    command: "python3 -m unittest discover -s app/api",
    files: {
      "app/api/test_contract.py": `import importlib.util\nfrom pathlib import Path\nimport sqlite3\nimport tempfile\nfrom types import SimpleNamespace\nimport unittest\n\nspec = importlib.util.spec_from_file_location('app', Path(__file__).with_name('${path.basename(item.entry)}'))\napp = importlib.util.module_from_spec(spec)\nspec.loader.exec_module(app)\n\nclass ContractTest(unittest.TestCase):\n    def test_existing_behavior(self):\n${body}`,
    },
  };
}

export function caseIdentity(item, reference = false) {
  return digest(caseFiles(item, reference));
}
