-- Review-based right-to-erasure / removal-request system.
-- removal_request: tracks public requests to hide a conviction-database entry.
-- hidden_conviction: a SET of target_refs that should not appear on the public
--   /database page. hide = INSERT OR IGNORE; unhide = DELETE. The conviction row
--   is never touched — this is the "nothing is hard-deleted" guarantee and it
--   works for both compile-time and database-backed entries.

CREATE TABLE IF NOT EXISTS removal_request (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  target_ref TEXT NOT NULL,          -- article slug identifying the conviction entry
  requester_name TEXT NOT NULL DEFAULT '',
  requester_email TEXT NOT NULL DEFAULT '',
  reason TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'received',
  created_at INTEGER NOT NULL,
  decided_at INTEGER,
  decision_note TEXT NOT NULL DEFAULT '',
  decided_by TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_removal_request_ref
  ON removal_request(target_ref);

CREATE TABLE IF NOT EXISTS hidden_conviction (
  target_ref TEXT PRIMARY KEY,
  removal_request_id INTEGER NOT NULL,
  hidden_at INTEGER NOT NULL,
  hidden_by TEXT NOT NULL DEFAULT ''
);
