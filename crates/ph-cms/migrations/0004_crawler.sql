-- Crawler ingest + court-watch schema (the INGEST half of the editorial
-- pipeline). Adds: a registry of external sources, a queue of crawled LEADS
-- awaiting editorial triage, a database-backed conviction table (approval-gated,
-- replacing the compile-time CONVICTIONS), and a PRIVATE court-watch store for
-- upcoming / appeal hearings.
--
-- ACTIVE-PROCEEDINGS FIREWALL (invariant): `court_watch` (live / upcoming
-- proceedings) and `ingest_item` / `conviction` (post-conviction, public) are
-- SEPARATE tables with NO foreign key and NO promotion path between them. A case
-- can enter the public pipeline only as a fresh post-conviction lead AFTER it
-- concludes — never by promoting a court_watch row. This is the Contempt of
-- Court Act 1981 boundary; it is enforced in code (no function reads court_watch
-- and writes ingest_item/conviction) and asserted by ph_cms tests.

-- A configured external source the crawler polls.
CREATE TABLE IF NOT EXISTS ingest_source (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  key TEXT NOT NULL UNIQUE,            -- stable adapter key, e.g. "caselaw" / "bbc-leicester"
  kind TEXT NOT NULL,                  -- "caselaw" | "news" | "courtwatch"
  label TEXT NOT NULL,                 -- human label for the desk
  url TEXT NOT NULL DEFAULT '',        -- feed / list URL
  enabled INTEGER NOT NULL DEFAULT 1,
  last_polled_at INTEGER               -- unix secs; NULL until first poll
);

-- A crawled LEAD awaiting editorial triage. PUBLIC pipeline (post-conviction /
-- news). Everything in `extracted_json` is UNVERIFIED machine output — an editor
-- writes our own report from the record; the legal gate clears reporting
-- restrictions before anything is published. `snippet` is a short extract only,
-- never the source's full body (copyright). `image_url` is a REFERENCE for the
-- editor — never auto-republished.
CREATE TABLE IF NOT EXISTS ingest_item (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source_id INTEGER NOT NULL,
  source_key TEXT NOT NULL DEFAULT '',           -- denormalised for display
  external_id TEXT NOT NULL,                     -- stable id from the source (guid/url)
  url TEXT NOT NULL,                             -- link-back to the source
  title TEXT NOT NULL,
  snippet TEXT NOT NULL DEFAULT '',              -- short extract ONLY
  offence_category TEXT NOT NULL DEFAULT 'unknown', -- sexual | child | other | unknown
  extracted_json TEXT NOT NULL DEFAULT '{}',     -- UNVERIFIED extracted fields
  image_url TEXT NOT NULL DEFAULT '',            -- source image REFERENCE only
  image_attribution TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'new',            -- new | triaged | promoted | dismissed
  promoted_article_id INTEGER,                   -- set when promoted to a draft article
  created_at INTEGER NOT NULL,
  UNIQUE (source_id, external_id)
);

-- A public conviction-database entry, with an approval lifecycle. Goes public
-- only after we publish our OWN legal-gated report (article_id required to
-- publish); the entry additionally cites the court-record / news source. This
-- table makes the formerly compile-time CONVICTIONS editable + approvable at
-- runtime.
CREATE TABLE IF NOT EXISTS conviction (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  area TEXT NOT NULL DEFAULT '',                 -- town/city, or '' if not stated
  offence TEXT NOT NULL,
  outcome TEXT NOT NULL DEFAULT '',              -- sentence summary
  date TEXT NOT NULL DEFAULT '',                 -- human, e.g. "May 2026"
  iso_date TEXT NOT NULL DEFAULT '',
  lat REAL NOT NULL DEFAULT 0.0,
  lng REAL NOT NULL DEFAULT 0.0,
  article_id INTEGER,                            -- our published report (required to publish)
  article_slug TEXT NOT NULL DEFAULT '',         -- denormalised slug for the public link
  source_url TEXT NOT NULL DEFAULT '',           -- court-record / news source (link-back)
  source_name TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'draft',          -- draft | published | retracted
  created_at INTEGER NOT NULL,
  published_at INTEGER
);

-- PRIVATE court-watch: upcoming / appeal / listing hearings the newsroom wants to
-- attend or request a transcript for. NEVER published; structurally isolated from
-- the public tables above (see the firewall note at the top).
CREATE TABLE IF NOT EXISTS court_watch (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  court TEXT NOT NULL DEFAULT '',
  case_ref TEXT NOT NULL DEFAULT '',
  hearing_date TEXT NOT NULL DEFAULT '',         -- as published (iso or human)
  hearing_type TEXT NOT NULL DEFAULT 'listing',  -- trial | appeal | sentencing | listing
  offence_category TEXT NOT NULL DEFAULT 'unknown',
  source_key TEXT NOT NULL DEFAULT '',
  external_id TEXT NOT NULL DEFAULT '',
  source_url TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'watching',       -- watching | attending | transcript_requested | closed
  created_at INTEGER NOT NULL,
  UNIQUE (source_key, external_id)
);
