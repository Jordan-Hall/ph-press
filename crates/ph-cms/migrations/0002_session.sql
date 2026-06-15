-- Staff editorial sessions for the /desk console. Only the SHA-256 of each token
-- is stored (the raw token lives in the operator's HttpOnly cookie), so a DB leak
-- never yields a usable session. Rows are pruned on validation past expires_at.

CREATE TABLE IF NOT EXISTS session (
  token_hash   TEXT PRIMARY KEY,
  user_id      INTEGER NOT NULL,
  username     TEXT NOT NULL,
  display_name TEXT NOT NULL,
  role         TEXT NOT NULL,
  created_at   INTEGER NOT NULL,
  expires_at   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_session_expires ON session (expires_at);
