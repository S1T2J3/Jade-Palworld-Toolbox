//! Reciprocal-rank fusion of the player's wording and a translation from the existing plan call.
use super::{
    AssistantReply, Data, array, catalog_text::CatalogText, reference_search, survival_guides, text,
};
use std::collections::{BTreeSet, HashMap, HashSet};

struct Entry {
    length: usize,
    caption: String,
    body: String,
    english: Option<String>,
    links: Vec<(String, String)>,
}

#[derive(Default)]
pub(super) struct Index {
    entries: Vec<Entry>,
    prose_ids: HashMap<String, usize>,
    postings: HashMap<String, Vec<(usize, u16)>>,
    average: f64,
}

fn words(text: &str) -> Vec<String> {
    let mut words: Vec<_> = reference_search::tokens(text).collect();
    let mut previous = None;
    for c in text.chars() {
        if ('\u{3400}'..='\u{9fff}').contains(&c) {
            if let Some(p) = previous {
                words.push(format!("{p}{c}"));
            }
            previous = Some(c);
        } else {
            previous = None;
        }
    }
    words
}

pub(crate) struct EvidencePassage {
    pub title: String,
    pub body: String,
}

/// Recognize complete passages actually sent to compose. A curated guide may be
/// displayed in its other shipped language; neither wording is model-generated.
pub(crate) fn offered_passages(d: &Data, question: &str, facts: &str) -> Vec<EvidencePassage> {
    if !d.ready("search-index-data") || !d.ready("item-effects-data") {
        return Vec::new();
    }
    // Numeric tables, exact routes and other structured calculations keep their
    // established synthesis contract. This lane covers sourced prose.
    let includes_guide = survival_guides::guides()
        .iter()
        .any(|g| facts.contains(&g.en) || facts.contains(&g.zh));
    if !includes_guide {
        return Vec::new();
    }
    let catalog = d.catalog_text.get_or_init(|| CatalogText::build(d));
    let index = catalog.bilingual.get_or_init(|| Index::build(d, catalog));
    let english = !question
        .chars()
        .any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let mut results = Vec::new();
    let asked = crate::assistant_engine::field_text(question);
    for entry in &index.entries {
        let at = [Some(entry.body.as_str()), entry.english.as_deref()]
            .into_iter()
            .flatten()
            .filter_map(|s| facts.find(s))
            .min();
        let Some(at) = at else {
            continue;
        };
        let Some((_, source)) = entry.links.first() else {
            continue;
        };
        let catalog_row = source.starts_with('#');
        let mut offered_at = at;
        if catalog_row {
            // An unnamed item's tooltip describes its bearer, not another
            // mechanic the player is asking about. Keep its name in scope.
            let name = entry.caption.split(['（', '(']).next().unwrap_or("").trim();
            let id = source.rsplit('/').next().unwrap_or("");
            if !(name.len() > 1 && asked.contains(&crate::assistant_engine::field_text(name))
                    || id.len() > 2 && asked.contains(&crate::assistant_engine::field_text(id)))
            {
                continue;
            }
            // A generic tooltip can be shared by several items. Bind it to the
            // row in the actual evidence rather than to a title elsewhere in it.
            let Some(position) = catalog_position(facts, &entry.caption, source, &entry.body) else { continue };
            offered_at = position;
        }
        let body = if english {
            entry.english.as_deref().unwrap_or(&entry.body)
        } else {
            &entry.body
        };
        if body.chars().count() <= 700 {
            results.push((offered_at, entry.caption.as_str(), body, catalog_row));
        }
    }
    results.sort_by_key(|(at, _, _, _)| *at);
    let mut seen = HashSet::new();
    results
        .into_iter()
        .filter(|(_, caption, body, catalog_row)| seen.insert((catalog_row.then_some(*caption), *body)))
        .take(12)
        .map(|(_, caption, body, catalog_row)| EvidencePassage {
            title: caption.trim_start_matches("生存指南 · ").into(),
            body: if catalog_row { format!("{caption}：{body}") } else { body.into() },
        })
        .collect()
}

fn catalog_position(facts: &str, caption: &str, route: &str, body: &str) -> Option<usize> {
    let inline = format!("{caption}：{body}");
    let linked = format!("{caption} · {route}\n{body}");
    [inline, linked].iter().filter_map(|row| {
        facts.match_indices(row).find(|(at, _)| *at == 0 || facts[..*at].ends_with('\n')).map(|(at, _)| at)
    }).min()
}

impl Index {
    fn insert(&mut self, text: String, mut entry: Entry) {
        let mut counts: HashMap<String, u16> = HashMap::new();
        for word in words(&text) {
            *counts.entry(word).or_default() += 1;
        }
        entry.length = counts.values().map(|&n| usize::from(n)).sum();
        let i = self.entries.len();
        for (word, count) in counts {
            self.postings.entry(word).or_default().push((i, count));
        }
        self.entries.push(entry);
    }

    fn build(d: &Data, catalog: &CatalogText) -> Self {
        let mut index = Self::default();
        for guide in survival_guides::guides() {
            index.prose_ids.insert(
                super::semantic::prose_key('g', &guide.zh),
                index.entries.len(),
            );
            let mut links = vec![(guide.title.clone(), guide.source.clone())];
            links.extend(
                guide
                    .also
                    .iter()
                    .map(|url| (guide.title.clone(), url.clone())),
            );
            if !guide.route.is_empty() {
                links.push((guide.label.clone(), guide.route.clone()));
            }
            index.insert(
                [
                    guide.title.as_str(),
                    &guide.aliases.join(" "),
                    &guide.zh,
                    &guide.en,
                ]
                .join(" "),
                Entry {
                    length: 0,
                    caption: format!("生存指南 · {}", guide.title),
                    body: guide.zh.clone(),
                    english: Some(guide.en.clone()),
                    links,
                },
            );
        }
        for (topic, section, tags, para, url) in reference_search::passages() {
            index
                .prose_ids
                .insert(super::semantic::prose_key('a', para), index.entries.len());
            index.insert(
                [topic, section, tags, para].join(" "),
                Entry {
                    length: 0,
                    caption: format!("ThePalWiki / Palworld Wiki · {topic} / {section}"),
                    body: para.into(),
                    english: None,
                    links: vec![(format!("ThePalWiki · {topic}"), url.into())],
                },
            );
        }
        for (row, description) in catalog.descriptions(d) {
            let name = text(row, "name");
            let kind = text(row, "kind");
            let aliases: Vec<_> = array(&row["aliases"])
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            let caption = if kind == "伙伴技能" {
                format!("{} · {name}", text(row, "hint"))
            } else {
                format!("{name}（{kind}）")
            };
            index.insert(
                format!("{caption} {} {description}", aliases.join(" ")),
                Entry {
                    length: 0,
                    caption: caption.clone(),
                    body: description.into(),
                    english: None,
                    links: vec![(caption, text(row, "href").into())],
                },
            );
        }
        index.average = index.entries.iter().map(|e| e.length as f64).sum::<f64>()
            / index.entries.len().max(1) as f64;
        index
    }

    fn rank(&self, terms: BTreeSet<String>) -> Vec<usize> {
        let mut scores: HashMap<usize, (usize, f64)> = HashMap::new();
        for term in terms {
            let Some(postings) = self.postings.get(&term) else {
                continue;
            };
            let idf = ((self.entries.len() as f64 + 1.) / (postings.len() as f64 + 0.5)).ln();
            if idf < 0.75 {
                continue;
            }
            for &(i, count) in postings {
                let tf = f64::from(count);
                let score = scores.entry(i).or_default();
                score.0 += 1;
                score.1 += idf * 2.2 * tf
                    / (tf + 1.2 * (0.25 + 0.75 * self.entries[i].length as f64 / self.average));
            }
        }
        let mut rows: Vec<_> = scores
            .into_iter()
            .filter(|(_, (hits, score))| *hits >= 2 && *score >= 6.)
            .collect();
        rows.sort_by(|a, b| b.1.1.total_cmp(&a.1.1).then(a.0.cmp(&b.0)));
        rows.into_iter().take(10).map(|(i, _)| i).collect()
    }

    // A nearest neighbour for a rare name can still describe an unrelated
    // mechanic. Prefer a retrieved passage with an actual rare query term.
    // Ignore planner-added terms here; they did not originate in the question.
    fn prefer_matching_subject(&self, ranked: &mut [(usize, f64)], question: &str, translation: &str) {
        let mut anchored = HashSet::new();
        for term in words(question.lines().next().unwrap_or("")).into_iter()
            .chain(words(translation.lines().next().unwrap_or(""))) {
            if let Some(rows) = self.postings.get(&term)
                && rows.len() * 25 <= self.entries.len() {
                anchored.extend(rows.iter().map(|&(id, _)| id));
            }
        }
        if !anchored.is_empty() {
            ranked.sort_by_key(|(id, _)| !anchored.contains(id));
        }
    }
}

pub(super) fn lookup(
    d: &Data,
    question: &str,
    terms: &[String],
    translation: &str,
) -> Option<AssistantReply> {
    if translation.trim().is_empty()
        || question.trim().is_empty()
        || !d.ready("search-index-data")
        || !d.ready("item-effects-data")
    {
        return None;
    }
    let catalog = d.catalog_text.get_or_init(|| CatalogText::build(d));
    let index = catalog.bilingual.get_or_init(|| Index::build(d, catalog));
    let asked = crate::assistant_engine::field_text(question);
    let translated = crate::assistant_engine::field_text(translation);
    let translated_rank = index.rank(words(&translated).into_iter().collect());
    let no_translation_match = translated_rank.is_empty();
    let mut original: BTreeSet<_> = words(&asked).into_iter().collect();
    original.extend(reference_search::query(question));
    for term in terms.iter().take(3) {
        original.extend(words(&crate::assistant_engine::field_text(term)));
    }
    let mut fused = HashMap::<usize, f64>::new();
    for ranks in [index.rank(original), translated_rank] {
        for (rank, i) in ranks.into_iter().enumerate() {
            *fused.entry(i).or_default() += 1. / (11. + rank as f64);
        }
    }
    let mut ranked: Vec<_> = fused.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    if let Some(semantic) = super::semantic::index(d) {
        let vector_rank = semantic
            .prose(question)
            .into_iter()
            .filter_map(|id| index.prose_ids.get(id).copied())
            .collect::<Vec<_>>();
        let mut scores = HashMap::new();
        for ids in [
            ranked.iter().take(10).map(|(i, _)| *i).collect::<Vec<_>>(),
            vector_rank,
        ] {
            for (rank, id) in ids.into_iter().enumerate() {
                *scores.entry(id).or_insert(0.) += 1.0 / (11 + rank) as f64;
            }
        }
        ranked = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    } else if no_translation_match {
        return None;
    }
    index.prefer_matching_subject(&mut ranked, &asked, &translated);
    let english = !asked
        .chars()
        .any(|c| ('\u{3400}'..='\u{9fff}').contains(&c));
    let mut snippets = Vec::new();
    let mut seen_bodies = HashSet::new();
    let mut links = Vec::new();
    for (i, _) in ranked {
        let e = &index.entries[i];
        let body = if english {
            e.english.as_deref().unwrap_or(&e.body)
        } else {
            &e.body
        };
        let subject = e.links.first().filter(|(_, link)| link.starts_with('#')).map(|_| &e.caption);
        if !seen_bodies.insert((subject, body)) {
            continue;
        }
        snippets.push(format!("{} · {}\n{}", e.caption, e.links.first()?.1, body));
        for link in &e.links {
            if !links.iter().any(|(_, route)| route == &link.1) {
                links.push(link.clone());
            }
        }
        if snippets.len() == 3 {
            break;
        }
    }
    (!snippets.is_empty()).then(|| AssistantReply {
        text: snippets.join("\n"),
        links,
        suggestions: Vec::new(),
    })
}
