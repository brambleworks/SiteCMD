-- Enforce one issue link per project, check, run, and provider attempt.
-- Deduplicate existing rows by keeping the newest before adding the unique index.

DELETE FROM issue_links
 WHERE id NOT IN (
   SELECT MAX(id)
     FROM issue_links
    GROUP BY project_id, check_id, run_id, provider
 );

CREATE UNIQUE INDEX idx_issue_links_attempt_identity
    ON issue_links(project_id, check_id, run_id, provider);
