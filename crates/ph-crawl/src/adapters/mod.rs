//! Source adapters. Each turns a fetched feed/page into the DB-agnostic shapes in
//! [`crate::source`], applying its own relevance + active-proceedings filtering:
//!
//! - [`caselaw`] — National Archives **Find Case Law** Atom feed → public leads.
//! - [`news`] — UK national / local **RSS/Atom** feeds → public leads (strict:
//!   concluded only).
//! - [`courtwatch`] — court-listing HTML → PRIVATE upcoming/appeal hearings.

pub mod caselaw;
pub mod courtwatch;
pub mod news;
