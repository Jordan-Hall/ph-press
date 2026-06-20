-- Email-based password recovery (/desk → forgot / reset). Staff accounts gain a
-- contact email (nullable; used for recovery and, later, notifications). Reset
-- tokens are single-use, expiring, and stored only as a SHA-256 hash — a DB leak
-- never yields a usable link. Existing rows get a NULL email until set.
ALTER TABLE staff_user ADD COLUMN email TEXT;

CREATE TABLE password_reset_token (
  id         INTEGER PRIMARY KEY,
  user_id    INTEGER NOT NULL REFERENCES staff_user(id),
  token_hash TEXT    NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  used_at    INTEGER
);
CREATE INDEX idx_prt_user ON password_reset_token(user_id);
