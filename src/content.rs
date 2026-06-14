//! Compile-time article store. Until the editorial CMS (WS2) wires a database,
//! published articles live here as data. Only org/explainer pieces are seeded
//! (true statements about the organisation); case reports wait for verified
//! court-record facts + the editorial workflow. Each article can carry an
//! optional self-hosted video (poster + sources) which drives the on-page
//! player AND the og:video / twitter:player tags so a shared link plays the
//! video on Facebook/social.

/// A self-hosted video attached to an article (served from /media; drives the
/// player + Open Graph video tags for rich social sharing).
#[derive(Clone, PartialEq)]
pub struct Video {
    pub mp4: &'static str,       // e.g. "/media/<slug>.mp4"
    pub poster: &'static str,    // e.g. "/media/<slug>.jpg"
    pub width: u32,
    pub height: u32,
    pub duration_secs: u32,
}

#[derive(Clone, PartialEq)]
pub struct Article {
    pub slug: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub byline: &'static str,
    pub date: &'static str,         // human, e.g. "June 2026"
    pub iso_date: &'static str,     // for <time>/JSON-LD, e.g. "2026-06-14"
    pub kind: &'static str,         // "Announcement" | "Explainer" | "Court report"
    pub body: &'static [&'static str], // paragraphs
    pub video: Option<Video>,
}

impl Article {
    pub fn og_image(&self) -> String {
        match &self.video {
            Some(v) => v.poster.to_string(),
            None => "/og.png".to_string(),
        }
    }
}

/// Published articles, newest first. Org/explainer only for now.
pub const ARTICLES: &[Article] = &[
    Article {
        slug: "predator-hunters-launches-its-newsroom",
        title: "Predator Hunters launches its newsroom",
        summary: "After nearly a decade on the front line, we are opening our newsroom: court reporting from the public record, a public conviction database, and the standards you can hold us to.",
        byline: "Jordan Upton",
        date: "June 2026",
        iso_date: "2026-06-14",
        kind: "Announcement",
        body: &[
            "For nearly ten years we have worked on the front line of child protection. It started with decoy operations: posing as children online to find the adults who go looking for them, gathering the evidence, and handing it to the police. That work is hard, and it taught us something no training could. We saw how grooming begins, how it escalates, and how the people behind it move from one app to the next to avoid being caught.",
            "Today we are opening our newsroom.",
            "From here we will report on the cases we have worked, once they have been through the courts, drawn from the public record. Alongside the reporting we are building a public database so a community can look up offenders who have been convicted, by name, area and offence. And we will keep doing the frontline work, with a smaller team, because it keeps us close to how this actually happens.",
            "Two lines have never moved, and they never will. We do not name anyone before they are charged. We hold footage back until there is a conviction, we censor it where it is needed, and we only run it when it genuinely helps people keep children safe.",
            "We are an independent, self-funded team. We work with the police, not in their place, and we are working towards registration with IMPRESS, the UK's approved press regulator, so you can hold us to a published standard. If you have information, want to support the work, or are a journalist or safeguarding partner, get in touch. If a child is in immediate danger, call 999.",
        ],
        video: None,
    },
    Article {
        slug: "how-we-report",
        title: "How we report, and the lines we won't cross",
        summary: "How a group that started by catching predators can also be a publisher people trust: we keep the catching and the reporting apart, and we never name anyone before a charge.",
        byline: "Jordan Upton",
        date: "June 2026",
        iso_date: "2026-06-14",
        kind: "Explainer",
        body: &[
            "People ask us, fairly, how a group that started by catching predators can also be a publisher people trust. The answer is in how we work, so here it is in plain words.",
            "We catch, and we report, but we keep the two apart. The frontline team runs decoy operations and, when it is safe, confronts the person and holds them for the police with everything we have gathered. What we choose to publish is a separate decision, made later, to a different standard.",
            "We never name anyone before they are charged. Not on the site, not in a video, not in a caption. Before a charge, an accusation can wreck an innocent life and collapse a real case, so we do not make one.",
            "We hold footage back until there is a conviction. When we do publish it, we censor what needs censoring, and we only run it when it teaches people something that helps keep children safe. We are not in it for a show.",
            "We report from the public court record, after a case has concluded, and we check what we write against that record. If we get something significantly wrong, we correct it promptly and with the same prominence as the original, and we keep both on the record.",
            "We are working towards registration with IMPRESS so these are not just our promises but a standard you can hold us to, with a complaints process that goes beyond us if you are not satisfied.",
        ],
        video: None,
    },
    Article {
        slug: "why-a-public-conviction-database",
        title: "Why we built a public conviction database",
        summary: "Court records are public but scattered. We think a community has a right to see what the courts have decided, in one place, in plain terms, drawn from the public record.",
        byline: "Jordan Upton",
        date: "June 2026",
        iso_date: "2026-06-14",
        kind: "Explainer",
        body: &[
            "Court records are public, but they are scattered. A conviction reported one afternoon in one courtroom is, in practice, almost impossible for an ordinary person to find again. We think a community has a right to see what the courts have decided, in one place, in plain terms.",
            "So we are building a public conviction database. You will be able to search by name, area or offence and find people who have been convicted, with the facts drawn from the public court record.",
            "It is bound by the same lines as everything else we do. Every entry comes only after a conviction, never while a case is live and never before a charge. Every entry is checked against the court record before it goes up. It is a record of what the courts have already decided in public, not an accusation of our own.",
            "There is no photo upload and no face search on the public side. You search with words, and you see the court's findings.",
            "If you believe an entry is wrong, out of date, or that something needs correcting or removing, you can tell us and we will check it against the record. We handle this data under UK data-protection law, with a documented basis and a clear route to have an entry reviewed.",
        ],
        video: None,
    },
];

pub fn by_slug(slug: &str) -> Option<&'static Article> {
    ARTICLES.iter().find(|a| a.slug == slug)
}
