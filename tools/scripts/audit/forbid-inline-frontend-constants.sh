#!/usr/bin/env bash
# Enforce the canonical time, severity, and score helpers.
set -e

SRC="apps/desktop/src"

# A missing source tree must not look like a successful no-match.
if [ ! -d "$SRC" ]; then
  echo "forbid-inline-frontend-constants: scanned directory missing: $SRC" >&2
  exit 2
fi

# Time formatting belongs in lib/format.ts.
violations=$(grep -rEn '(\* 60 *\* *60 *\* *1000|/ ?60_000|/ ?3_?600_?000|/ ?86_?400_?000)' \
  "$SRC" --include='*.ts' --include='*.tsx' \
  | grep -v "$SRC/lib/format" \
  | grep -v '\.test\.' \
  || true)
if [ -n "$violations" ]; then
  echo "Inline time-ago math is forbidden - import formatRelativeTime from @/lib/format:"
  echo "$violations"
  exit 1
fi

# Severity ordering belongs in lib/severity.ts.
violations=$(grep -rEn 'critical: *0,? *high: *1' \
  "$SRC" --include='*.ts' --include='*.tsx' \
  | grep -v "$SRC/lib/severity" \
  | grep -v '\.test\.' \
  || true)
if [ -n "$violations" ]; then
  echo "Inline SEVERITY_ORDER / RANK is forbidden -- import from @/lib/severity:"
  echo "$violations"
  exit 1
fi

# Score bands belong in lib/score.ts.
violations=$(grep -rEn 'score *>= *90|score *>= *70|score *>= *50' \
  "$SRC" --include='*.ts' --include='*.tsx' \
  | grep -v "$SRC/lib/score" \
  | grep -v '\.test\.' \
  || true)
if [ -n "$violations" ]; then
  echo "Inline score-band thresholds are forbidden -- import from @/lib/score:"
  echo "$violations"
  exit 1
fi
