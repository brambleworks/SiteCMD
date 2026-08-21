-- Persist checkout identity with immutable code evidence so delayed uploads
-- describe the scanned tree, not the current checkout.
ALTER TABLE scan_runs ADD COLUMN code_commit_sha TEXT;
ALTER TABLE scan_runs ADD COLUMN code_tree_clean INTEGER
  CHECK (code_tree_clean IS NULL OR code_tree_clean IN (0, 1));

CREATE INDEX idx_scan_runs_code_provenance
  ON scan_runs (project_id, environment_scope_key, code_commit_sha)
  WHERE source = 'code_scan';
