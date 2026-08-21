-- Persist one revisioned route scope shared by manual and scheduled scans.
-- Store canonical paths only; scope_revision rejects stale connected writes.

CREATE TABLE IF NOT EXISTS site_scan_scope (
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    route TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (site_id, route)
);

CREATE INDEX IF NOT EXISTS idx_site_scan_scope_site
    ON site_scan_scope(site_id, position);

ALTER TABLE sites ADD COLUMN scope_revision INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sites ADD COLUMN scope_updated_at TEXT;
