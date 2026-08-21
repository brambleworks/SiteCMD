import { LIFECYCLE_VALIDATION } from "./guardrail-license-sources.mjs";
import { stripNonCode } from "./guardrail-source-text.mjs";

const PATH = LIFECYCLE_VALIDATION;

export function licenseValidationBranchFailures(read) {
  const raw = read(PATH);
  // Find the string-literal anchor in raw text, then inspect code at the same
  // offsets in the length-preserving stripped view.
  const source = stripNonCode(raw, PATH);
  const failures = [];

  // One shared window prevents sibling branches or source-pin strings from
  // satisfying any arm check. Its reach covers the complete current branch.
  const branchStart = raw.indexOf('tracing::warn!("License validation failed');
  const branch = branchStart === -1 ? "" : source.slice(branchStart, branchStart + 4000);

  // The present-row arm must answer from the reread row through the grace ladder.
  if (
    branchStart === -1 ||
    !/Ok\(Some\(row\)\) => \{[\s\S]{0,700}?offline_validation_or_downgrade\(&row\)/.test(branch)
  ) {
    failures.push(
      `${PATH} validate_license's failed-validation branch must answer the still-installed row through offline_validation_or_downgrade(&row) so the cached tier never silently drops to Free and never resurrects the pre-request capture`,
    );
  }

  // Pin distinct answers for replaced, absent, and unreadable rows.
  if (
    !/row\.instance_id != state\.instance_id/.test(branch) ||
    !/Ok\(None\) => \{[\s\S]{0,400}?free_info_result\(\)/.test(branch) ||
    !/Err\(error\) => \{[\s\S]{0,600}?offline_validation_or_downgrade\(&state\)/.test(branch)
  ) {
    failures.push(
      `${PATH} validate_license's failed-validation branch must note an instance change, answer Free only for a genuinely absent row, and reserve the captured-state fallback for a failed re-read`,
    );
  }
  return failures;
}
