import { describe, it } from "vitest";
import {
  GUARDRAIL_TEST_TIMEOUT_MS,
  expectGuardrailFailure,
  mustMutate,
  readFixtureFile,
  writeFixtureFile,
  rules,
} from "./guardrail-test-support.mjs";

describe.concurrent(
  "Google OAuth release contracts",
  { timeout: GUARDRAIL_TEST_TIMEOUT_MS },
  () => {
    it.each(["exchange", "refresh"])(
      "requires the client credential in the %s form",
      (operation) => {
        expectGuardrailFailure(
          rules.desktopOAuthSafetyFailures,
          (root) => {
            const file = "apps/desktop/src-tauri/src/integrations/google_oauth.rs";
            const source = readFixtureFile(root, file);
            const boundary = source.indexOf(`fn build_token_${operation}_form`);
            const end = source.indexOf("\n}", boundary) + 2;
            writeFixtureFile(
              root,
              file,
              source.slice(0, boundary) +
                mustMutate(
                  source.slice(boundary, end),
                  'form.push(("client_secret", secret));',
                  'form.push(("unused", secret));',
                ) +
                source.slice(end),
            );
          },
          "include it in both token exchange and refresh requests",
        );
      },
    );

    it.each(["exchange", "refresh"])(
      "requires loading the credential for %s requests",
      (operation) => {
        expectGuardrailFailure(
          rules.desktopOAuthSafetyFailures,
          (root) => {
            const file = "apps/desktop/src-tauri/src/integrations/google_oauth.rs";
            const source = readFixtureFile(root, file);
            const pattern = new RegExp(
              `(build_token_${operation}_form\\(\\s*client_id,\\s*)client_secret\\(\\)`,
              "g",
            );
            writeFixtureFile(root, file, mustMutate(source, pattern, "$1None"));
          },
          "include it in both token exchange and refresh requests",
        );
      },
    );

    it.each(["GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"])(
      "requires %s in the release workflow",
      (name) => {
        expectGuardrailFailure(
          rules.releaseWorkflowSafetyFailures,
          (root) => {
            const file = ".github/workflows/release.yml";
            const source = readFixtureFile(root, file);
            writeFixtureFile(
              root,
              file,
              mustMutate(source, `          ${name}: \${{ secrets.${name} }}\n`, ""),
            );
          },
          "supply both Google Desktop OAuth credentials",
        );
      },
    );

    it("requires the release credential preflight", () => {
      expectGuardrailFailure(
        rules.releaseWorkflowSafetyFailures,
        (root) => {
          const file = ".github/scripts/release/build-tauri-app.sh";
          writeFixtureFile(
            root,
            file,
            mustMutate(
              readFixtureFile(root, file),
              "node tools/scripts/check-google-oauth-config.mjs\n",
              "",
            ),
          );
        },
        "validate them before packaging",
      );
    });
  },
);
