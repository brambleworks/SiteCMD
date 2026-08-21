-- Remove the retired local scan quota; execution admission is idempotency-only.

DROP INDEX IF EXISTS idx_scan_executions_quota;

ALTER TABLE scan_executions DROP COLUMN quota_date;
ALTER TABLE scan_executions DROP COLUMN quota_state;
ALTER TABLE scan_executions DROP COLUMN counts_toward_quota;
