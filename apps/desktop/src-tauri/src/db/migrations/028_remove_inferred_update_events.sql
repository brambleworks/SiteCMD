-- Automatic dependency refreshes could mistake an unavailable update for an
-- applied update. Explicit verification events use a different source prefix.
DELETE FROM events
WHERE event_type = 'update'
  AND source = 'internal'
  AND source_id LIKE 'updates-refresh:%';
