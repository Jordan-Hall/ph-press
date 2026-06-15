//! Tamper-evident, hash-chained audit log for the PH Press editorial system.
//!
//! Every editorial action (submit, edit, legal sign-off, publish, correct,
//! retract, complaint logged, database entry changed) is appended as an [`Entry`]
//! whose hash commits to the previous entry's hash. Any later edit to a past
//! entry breaks every hash after it, so [`AuditChain::verify`] detects tampering.
//! This is what lets us prove, for IMPRESS, that the record was not altered after
//! the fact. Storage-agnostic: the CMS persists entries (SQLite) and rebuilds the
//! chain to verify.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Genesis `prev_hash` for the first entry (64 hex zeros).
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One immutable record in the chain.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub seq: u64,          // position in the chain, from 0
    pub ts: i64,           // unix seconds
    pub actor: String,     // who acted (staff username, or "system")
    pub action: String,    // what (e.g. "article.publish", "complaint.logged")
    pub subject: String,   // on what (e.g. an article slug or id)
    pub detail: String,    // free-form note or JSON payload
    pub prev_hash: String, // hash of the previous entry (or GENESIS)
    pub hash: String,      // sha256 over all of the above
}

impl Entry {
    /// Recompute this entry's hash from its fields (fields separated by a unit
    /// separator so field boundaries can't be forged by concatenation).
    pub fn compute_hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.seq.to_be_bytes());
        h.update(self.ts.to_be_bytes());
        for field in [
            self.actor.as_str(),
            self.action.as_str(),
            self.subject.as_str(),
            self.detail.as_str(),
            self.prev_hash.as_str(),
        ] {
            h.update(field.as_bytes());
            h.update([0x1f]);
        }
        hex(&h.finalize())
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// An append-only chain of [`Entry`]s.
#[derive(Clone, Debug, Default)]
pub struct AuditChain {
    entries: Vec<Entry>,
}

impl AuditChain {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Rebuild a chain from persisted entries (e.g. read back from SQLite),
    /// verifying it as we go.
    pub fn from_entries(entries: Vec<Entry>) -> Result<Self, VerifyError> {
        let chain = Self { entries };
        chain.verify()?;
        Ok(chain)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Hash of the latest entry, or GENESIS if empty.
    pub fn tip(&self) -> &str {
        self.entries
            .last()
            .map(|e| e.hash.as_str())
            .unwrap_or(GENESIS)
    }

    /// Append a new record and return it.
    pub fn append(
        &mut self,
        ts: i64,
        actor: impl Into<String>,
        action: impl Into<String>,
        subject: impl Into<String>,
        detail: impl Into<String>,
    ) -> &Entry {
        let mut e = Entry {
            seq: self.entries.len() as u64,
            ts,
            actor: actor.into(),
            action: action.into(),
            subject: subject.into(),
            detail: detail.into(),
            prev_hash: self.tip().to_string(),
            hash: String::new(),
        };
        e.hash = e.compute_hash();
        self.entries.push(e);
        self.entries.last().unwrap()
    }

    /// Verify the whole chain: every entry's seq, link to the previous hash, and
    /// its own hash must all check out. Returns the index of the first bad entry.
    pub fn verify(&self) -> Result<(), VerifyError> {
        let mut prev = GENESIS.to_string();
        for (i, e) in self.entries.iter().enumerate() {
            if e.seq != i as u64 {
                return Err(VerifyError::Seq(i));
            }
            if e.prev_hash != prev {
                return Err(VerifyError::Link(i));
            }
            if e.hash != e.compute_hash() {
                return Err(VerifyError::Hash(i));
            }
            prev = e.hash.clone();
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// Entry at index has the wrong sequence number.
    Seq(usize),
    /// Entry at index does not link to the previous entry's hash.
    Link(usize),
    /// Entry at index has been altered (hash mismatch).
    Hash(usize),
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::Seq(i) => write!(f, "audit chain: wrong sequence at entry {i}"),
            VerifyError::Link(i) => write!(f, "audit chain: broken link at entry {i}"),
            VerifyError::Hash(i) => write!(f, "audit chain: tampered entry {i}"),
        }
    }
}
impl std::error::Error for VerifyError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_verifies() {
        let mut c = AuditChain::new();
        c.append(1, "jordan", "article.submit", "david-coote-convicted", "");
        c.append(
            2,
            "scott",
            "article.legal_signoff",
            "david-coote-convicted",
            "ok",
        );
        c.append(3, "scott", "article.publish", "david-coote-convicted", "");
        assert_eq!(c.entries().len(), 3);
        assert!(c.verify().is_ok());
        assert_ne!(c.tip(), GENESIS);
    }

    #[test]
    fn detects_tampering() {
        let mut c = AuditChain::new();
        c.append(1, "jordan", "article.submit", "x", "");
        c.append(2, "scott", "article.publish", "x", "");
        // Forge a past entry's detail without recomputing hashes.
        c.entries[0].detail = "secretly changed".into();
        assert_eq!(c.verify(), Err(VerifyError::Hash(0)));
    }

    #[test]
    fn detects_reorder() {
        let mut c = AuditChain::new();
        c.append(1, "a", "x", "s", "");
        c.append(2, "b", "y", "s", "");
        c.entries.swap(0, 1);
        assert!(c.verify().is_err());
    }
}
