-- A Code fix occurrence is stable by canonical group plus relative path. Line
-- is a display snapshot and can move as the agent edits the file.

-- Existing dogfood rows may contain two active attempts for the same path at
-- different line snapshots. Keep the newest and settle older collisions before
-- tightening the unique index.
UPDATE fix_attempts AS older
SET status = 'canceled'
WHERE older.status IN ('briefed', 'verify_requested', 'verifying')
  AND EXISTS (
      SELECT 1
      FROM fix_attempts AS newer
      WHERE newer.project_id = older.project_id
        AND newer.env_url = older.env_url
        AND newer.check_id = older.check_id
        AND newer.target_kind = older.target_kind
        AND newer.target_relative_path IS older.target_relative_path
        AND newer.status IN ('briefed', 'verify_requested', 'verifying')
        AND (
            newer.updated_at > older.updated_at
            OR (newer.updated_at = older.updated_at AND newer.id > older.id)
        )
  );

DROP INDEX IF EXISTS uq_fix_attempts_active;

CREATE UNIQUE INDEX uq_fix_attempts_active
    ON fix_attempts(
        project_id, env_url, check_id, target_kind,
        COALESCE(target_relative_path, '')
    )
    WHERE status IN ('briefed', 'verify_requested', 'verifying');
