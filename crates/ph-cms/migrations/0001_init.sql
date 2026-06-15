-- Initial editorial CMS schema. Versioned: sqlx tracks applied migrations in
-- _sqlx_migrations and runs each exactly once, so deploys never recreate or wipe
-- existing data. Add future schema changes as new files (0002_*.sql, ...).

CREATE TABLE IF NOT EXISTS staff_user (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  role TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  totp_secret TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS article (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  slug TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  body TEXT NOT NULL,
  byline TEXT NOT NULL,
  kind TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'draft',
  is_ai_assisted INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  published_at INTEGER
);

CREATE TABLE IF NOT EXISTS review_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  article_id INTEGER NOT NULL,
  from_state TEXT NOT NULL,
  to_state TEXT NOT NULL,
  actor TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  ts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS correction (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  article_id INTEGER NOT NULL,
  original TEXT NOT NULL,
  corrected TEXT NOT NULL,
  reason TEXT NOT NULL DEFAULT '',
  ts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS complaint (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  article_slug TEXT NOT NULL DEFAULT '',
  complainant TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'received',
  ts INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS audit (
  seq INTEGER PRIMARY KEY,
  ts INTEGER NOT NULL,
  actor TEXT NOT NULL,
  action TEXT NOT NULL,
  subject TEXT NOT NULL,
  detail TEXT NOT NULL,
  prev_hash TEXT NOT NULL,
  hash TEXT NOT NULL
);
