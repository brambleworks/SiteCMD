-- Preserve first-seen and resolution scan IDs independently of latest observation.
-- Missing causal scans remain NULL rather than receiving guessed provenance.
ALTER TABLE work_items ADD COLUMN first_seen_scan_ref INTEGER;
ALTER TABLE work_items ADD COLUMN resolved_scan_ref INTEGER;

CREATE INDEX IF NOT EXISTS idx_work_items_resolved_scan_ref
    ON work_items(source, resolved_scan_ref)
    WHERE resolved_scan_ref IS NOT NULL;
