-- Topical section for articles, so stories created in /desk carry their own
-- section (Crime/Courts/Local/Community) on the live public feed instead of one
-- derived from the format. Existing rows default to 'News'; the public site
-- renders the compile-time seeds from content.rs (their real sections), so this
-- default is never surfaced for them.

ALTER TABLE article ADD COLUMN section TEXT NOT NULL DEFAULT 'News';
