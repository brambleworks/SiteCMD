-- Preserve producer verdict and confidence independently of severity.
-- Conservatively backfill existing non-passing rows as Fail.
ALTER TABLE work_items ADD COLUMN check_status TEXT
    CHECK (check_status IS NULL OR check_status IN ('pass', 'fail', 'warn', 'skipped'));
ALTER TABLE work_items ADD COLUMN confidence_reason TEXT;

UPDATE work_items
SET check_status = 'fail'
WHERE source IN ('web_scan', 'site_scan')
  AND check_status IS NULL;
