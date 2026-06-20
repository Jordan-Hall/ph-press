-- SEO + social fields for articles created/edited in /desk: a search-result
-- meta description (distinct from the on-page standfirst), a social/OG image
-- URL, and topic tags (JSON array of strings). Existing rows default to empty;
-- the public renderer falls back to the summary when meta_description is blank.
ALTER TABLE article ADD COLUMN meta_description TEXT NOT NULL DEFAULT '';
ALTER TABLE article ADD COLUMN og_image_url     TEXT NOT NULL DEFAULT '';
ALTER TABLE article ADD COLUMN tags             TEXT NOT NULL DEFAULT '[]';
