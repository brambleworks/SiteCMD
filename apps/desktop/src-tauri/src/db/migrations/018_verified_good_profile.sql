-- Store verified-good and drifted values separately per site and fact family.
-- Drift never overwrites the reference value; profile_revision changes only
-- when the profile moves.

CREATE TABLE IF NOT EXISTS site_verified_good (
    site_id INTEGER NOT NULL REFERENCES sites(id) ON DELETE CASCADE,
    field TEXT NOT NULL,
    good_value_json TEXT NOT NULL,
    good_digest TEXT NOT NULL,
    good_profile_version INTEGER NOT NULL,
    good_recorded_at INTEGER NOT NULL,
    good_source_scan_id INTEGER,
    good_origin TEXT NOT NULL
        CHECK(good_origin IN ('seeded', 'promoted', 'accepted', 'reseeded')),
    drift_value_json TEXT,
    drift_digest TEXT,
    drift_first_seen_at INTEGER,
    drift_last_seen_at INTEGER,
    drift_source_scan_id INTEGER,
    drift_dismissed INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (site_id, field)
);

CREATE INDEX IF NOT EXISTS idx_site_verified_good_site
    ON site_verified_good(site_id);

ALTER TABLE sites ADD COLUMN profile_revision INTEGER NOT NULL DEFAULT 0;
