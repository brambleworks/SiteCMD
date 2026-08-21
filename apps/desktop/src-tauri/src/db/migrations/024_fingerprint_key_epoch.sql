-- Track current and locally pending fingerprint-key versions.
-- Key material remains in the OS keychain; remote-machine pending claims are not stored here.

ALTER TABLE connected_sites ADD COLUMN fingerprint_key_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE connected_sites ADD COLUMN pending_key_version INTEGER;
