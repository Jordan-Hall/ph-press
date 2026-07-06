-- Small key/value store for operator-set runtime settings that must be editable
-- from the desk without a rebuild. First use: `regulator_registered` — whether we
-- are a registered member of our press regulator (IMPRESS). Absent/"0"/"false" is
-- read as false (the cautious default), so a fresh DB never over-claims. Every
-- change to this value is also written to the tamper-evident `audit` chain.

CREATE TABLE IF NOT EXISTS setting (
  key        TEXT PRIMARY KEY,
  value      TEXT NOT NULL,
  updated_at INTEGER NOT NULL DEFAULT 0
);
