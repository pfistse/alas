SELECT id,
  name,
  mtime_secs,
  usn,
  config
FROM deck_config
WHERE id = ?