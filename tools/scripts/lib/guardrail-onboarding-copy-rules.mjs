const ONBOARDING_COPY_FILES = [
  "apps/desktop/src/components/layout/PageGuide.tsx",
  "apps/desktop/src/app/FirstRunWalkthrough.tsx",
];

const BANNED_LINE_PATTERNS = [
  {
    re: /\bLaunch\b/,
    reason:
      'names the retired Launch page (removed with the launch-checklist pillar); lowercase "launch" concept copy is fine',
  },
  {
    re: /\bAction Items\b/,
    reason:
      'points at an "Action Items" label the dashboard does not render; it shows "Issues" and "Updates" cards',
  },
  {
    re: /\bqueues?\b/i,
    reason: 'user-facing copy says "list", not "queue"',
  },
];

export function onboardingCopyFailures(read) {
  const failures = [];
  for (const file of ONBOARDING_COPY_FILES) {
    const lines = read(file).split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      for (const { re, reason } of BANNED_LINE_PATTERNS) {
        if (re.test(lines[i])) {
          failures.push(`${file}:${i + 1} - ${reason}. Line: ${lines[i].trim()}`);
        }
      }
    }
  }
  return failures;
}
