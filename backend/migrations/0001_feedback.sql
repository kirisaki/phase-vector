CREATE TABLE submissions (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  submission_id TEXT NOT NULL,
  kind TEXT NOT NULL CHECK(kind IN ('feedback', 'result')),
  fingerprint TEXT NOT NULL,
  status TEXT NOT NULL CHECK(status IN ('pending', 'complete')),
  created_at INTEGER NOT NULL,
  payload_json TEXT NOT NULL,
  image_key TEXT,
  image_expires_at INTEGER,
  UNIQUE(session_id, submission_id)
);
CREATE INDEX submissions_expiry ON submissions(image_expires_at) WHERE image_key IS NOT NULL;
CREATE INDEX submissions_pending ON submissions(created_at) WHERE status = 'pending';
CREATE TABLE rate_limits (
  key TEXT PRIMARY KEY,
  count INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);
