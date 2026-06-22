-- IMPRESS article-complaint handling. Each complaint gains the complainant's
-- email (to acknowledge + reply), the IMPRESS Standards Code clause it concerns,
-- and acknowledgement/resolution timestamps for the 7-day / 21-day targets. A
-- message thread records internal staff notes and replies sent to the complainant.
ALTER TABLE complaint ADD COLUMN complainant_email TEXT NOT NULL DEFAULT '';
ALTER TABLE complaint ADD COLUMN category          TEXT NOT NULL DEFAULT '';
ALTER TABLE complaint ADD COLUMN acknowledged_at   INTEGER;
ALTER TABLE complaint ADD COLUMN resolved_at       INTEGER;

-- Map the old free statuses onto the IMPRESS-aligned set.
UPDATE complaint SET status = 'under_investigation' WHERE status = 'under_review';
UPDATE complaint SET status = 'not_upheld'          WHERE status = 'rejected';

CREATE TABLE complaint_message (
  id           INTEGER PRIMARY KEY,
  complaint_id INTEGER NOT NULL REFERENCES complaint(id),
  author       TEXT    NOT NULL, -- staff username, or 'complainant' / 'system'
  channel      TEXT    NOT NULL, -- 'internal' (staff-only) | 'reply' (emailed to complainant)
  body         TEXT    NOT NULL,
  ts           INTEGER NOT NULL
);
CREATE INDEX idx_complaint_message_cid ON complaint_message(complaint_id);
