# Release workflow helpers

These Bash scripts hold the substantial build, signing, and verification steps
called by `release.yml`. They run from the repository root and receive all
inputs through each workflow step's `env` block.

The credentialed publication logic remains inline in the workflow. Its job does
not check out repository code, so publishing cannot execute a script from the
source tree.

Run `pnpm exec vitest run tools/scripts/release-workflow-scripts.test.mjs` to
check workflow wiring and Bash syntax.
