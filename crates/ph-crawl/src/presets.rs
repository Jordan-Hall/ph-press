//! Default source presets, so the crawler can be switched on with just
//! `PH_CRAWL_ENABLED=1`. Each kind is overridable via its `PH_CRAWL_*_FEEDS`
//! env var; when that var is unset/empty these defaults are used (court-watch has
//! no default — it is opt-in because listing sources are site-specific / paid).

use crate::source::SourceConfig;

/// National Archives **Find Case Law** — the sanctioned Atom feed, queried for our
/// remit. Judgments are concluded matters; the extractor still filters relevance.
pub fn caselaw() -> Vec<SourceConfig> {
    vec![
        SourceConfig::new(
            "caselaw-sexual",
            "caselaw",
            "Find Case Law — sexual offences",
            "https://caselaw.nationalarchives.gov.uk/atom.xml?query=sexual+offences&order=-date",
        ),
        SourceConfig::new(
            "caselaw-child",
            "caselaw",
            "Find Case Law — offences against children",
            "https://caselaw.nationalarchives.gov.uk/atom.xml?query=indecent+images+of+children&order=-date",
        ),
    ]
}

/// UK regional news (the newsroom's local patch) — BBC England regional RSS.
pub fn news() -> Vec<SourceConfig> {
    ["leicester", "nottingham", "derbyshire"]
        .into_iter()
        .map(|r| {
            let cap = {
                let mut c = r.chars();
                c.next()
                    .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                    .unwrap_or_default()
            };
            SourceConfig::new(
                &format!("bbc-{r}"),
                "news",
                &format!("BBC News — {cap}"),
                &format!("https://feeds.bbci.co.uk/news/england/{r}/rss.xml"),
            )
        })
        .collect()
}

/// UK **police forces + national bodies**. Territorial forces on the GOSS /
/// Police.UK CMS are SCRAPED from their news listing (kind `police`); the Met and
/// the NCA expose RSS, parsed by the news adapter (kind `news`). Each adapter runs
/// the strict post-conviction sex/child filter, so only relevant convictions
/// become leads — appeals / missing-person / ongoing items are dropped.
///
/// Only listing URLs verified to serve the scrapeable structure are included.
/// Forces on other platforms or that block scraping (e.g. Lancashire, Humberside,
/// Dorset, Wiltshire, Avon & Somerset, Devon & Cornwall, Norfolk, Bedfordshire,
/// Cumbria, West Yorkshire) and Police Scotland / PSNI are not yet covered.
pub fn police() -> Vec<SourceConfig> {
    // (key, force label, verified GOSS news-listing URL)
    const FORCES: &[(&str, &str, &str)] = &[
        ("pol-leicestershire", "Leicestershire Police", "https://www.leics.police.uk/news/leicestershire/news/"),
        ("pol-derbyshire", "Derbyshire Constabulary", "https://www.derbyshire.police.uk/news/derbyshire/news/"),
        ("pol-nottinghamshire", "Nottinghamshire Police", "https://www.nottinghamshire.police.uk/news/nottinghamshire/news/"),
        ("pol-warwickshire", "Warwickshire Police", "https://www.warwickshire.police.uk/news/warwickshire/news/"),
        ("pol-staffordshire", "Staffordshire Police", "https://www.staffordshire.police.uk/news/staffordshire/news/"),
        ("pol-west-midlands", "West Midlands Police", "https://www.westmidlands.police.uk/news/"),
        ("pol-west-mercia", "West Mercia Police", "https://www.westmercia.police.uk/news/west-mercia/news/"),
        ("pol-merseyside", "Merseyside Police", "https://www.merseyside.police.uk/news/merseyside/news/"),
        ("pol-cheshire", "Cheshire Constabulary", "https://www.cheshire.police.uk/news/cheshire/news/"),
        ("pol-durham", "Durham Constabulary", "https://www.durham.police.uk/news/durham/news/"),
        ("pol-northumbria", "Northumbria Police", "https://www.northumbria.police.uk/news/northumbria/news/"),
        ("pol-sussex", "Sussex Police", "https://www.sussex.police.uk/news/sussex/news/"),
        ("pol-kent", "Kent Police", "https://www.kent.police.uk/news/kent/latest/"),
        ("pol-essex", "Essex Police", "https://www.essex.police.uk/news/essex/news/"),
        ("pol-surrey", "Surrey Police", "https://www.surrey.police.uk/news/surrey/news/"),
        ("pol-hampshire", "Hampshire Constabulary", "https://www.hampshire.police.uk/news/hampshire/news/"),
        ("pol-gloucestershire", "Gloucestershire Constabulary", "https://www.gloucestershire.police.uk/news/gloucestershire/news/"),
        ("pol-suffolk", "Suffolk Constabulary", "https://www.suffolk.police.uk/news/suffolk/news/"),
        ("pol-cambridgeshire", "Cambridgeshire Constabulary", "https://www.cambs.police.uk/news/cambridgeshire/news/"),
        ("pol-hertfordshire", "Hertfordshire Constabulary", "https://www.herts.police.uk/news/hertfordshire/news/"),
        ("pol-northamptonshire", "Northamptonshire Police", "https://www.northants.police.uk/news/northants/news/"),
        ("pol-lincolnshire", "Lincolnshire Police", "https://www.lincs.police.uk/news/lincolnshire/news/"),
        ("pol-cleveland", "Cleveland Police", "https://www.cleveland.police.uk/news/cleveland/news/"),
        ("pol-north-yorkshire", "North Yorkshire Police", "https://www.northyorkshire.police.uk/news/north-yorkshire/news/"),
        ("pol-south-yorkshire", "South Yorkshire Police", "https://www.southyorkshire.police.uk/news/south-yorkshire/news/"),
        ("pol-greater-manchester", "Greater Manchester Police", "https://www.gmp.police.uk/news/greater-manchester/news/"),
        ("pol-thames-valley", "Thames Valley Police", "https://www.thamesvalley.police.uk/news/thames-valley/news/"),
        ("pol-south-wales", "South Wales Police", "https://www.south-wales.police.uk/news/south-wales/news/"),
        ("pol-dyfed-powys", "Dyfed-Powys Police", "https://www.dyfed-powys.police.uk/news/dyfed-powys/news/"),
    ];
    let mut v: Vec<SourceConfig> = FORCES
        .iter()
        .map(|(k, l, u)| SourceConfig::new(k, "police", l, u))
        .collect();
    // National bodies — RSS, so parsed by the (strict, concluded-only) news adapter.
    v.push(SourceConfig::new(
        "pol-met",
        "news",
        "Metropolitan Police",
        "https://news.met.police.uk/rss/current_news/66871",
    ));
    v.push(SourceConfig::new(
        "nca",
        "news",
        "National Crime Agency",
        "https://www.nationalcrimeagency.gov.uk/news?format=feed&type=rss",
    ));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn police_preset_well_formed() {
        let p = police();
        // 29 scraped forces + Met + NCA
        assert_eq!(p.iter().filter(|s| s.kind == "police").count(), 29);
        assert_eq!(p.iter().filter(|s| s.kind == "news").count(), 2);
        assert!(p.iter().all(|s| s.url.starts_with("https://") && !s.key.is_empty()));
    }

    #[test]
    fn presets_are_well_formed() {
        for s in caselaw() {
            assert_eq!(s.kind, "caselaw");
            assert!(s
                .url
                .starts_with("https://caselaw.nationalarchives.gov.uk/atom.xml"));
        }
        let news = news();
        assert_eq!(news.len(), 3);
        assert!(news
            .iter()
            .all(|s| s.kind == "news" && s.url.contains("bbci.co.uk")));
        assert_eq!(news[0].label, "BBC News — Leicester");
    }
}
