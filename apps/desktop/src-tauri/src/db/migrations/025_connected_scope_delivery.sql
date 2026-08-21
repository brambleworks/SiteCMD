-- Track the highest acknowledged scope revision so failed delivery remains retryable.
ALTER TABLE connected_sites
    ADD COLUMN scope_synced_revision INTEGER NOT NULL DEFAULT 0
    CHECK (scope_synced_revision >= 0);
