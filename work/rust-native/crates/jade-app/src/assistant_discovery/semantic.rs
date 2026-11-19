//! Offline WordPiece mean embeddings. Scores only order candidates. Renderers
//! resolve every result back into its current article or verified game-table row.
use super::{Data, catalog_text::CatalogText};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, ops::Range, path::Path};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
mod relation;

const WIDTH: usize = 256;
const DIGEST: &str = include_str!("../../../../data/assistant-semantic-index.sha256");
pub(super) fn prose_key(kind: char, body: &str) -> String {
    format!(
        "{kind}:{}",
        &format!("{:x}", Sha256::digest(body.as_bytes()))[..16]
    )
}
pub(super) struct Index {
    file: Vec<u8>,
    vocab: HashMap<String, usize>,
    keys: Vec<String>,
    scales: Range<usize>,
    tokens: Range<usize>,
    doc_scales: Range<usize>,
    docs: Range<usize>,
}
fn read_u32(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
}
fn scale(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
impl Index {
    fn load(root: &Path) -> Option<Self> {
        let path = root.join("assistant-semantic-index.bin");
        if std::fs::metadata(&path).ok()?.len() > 32 * 1024 * 1024 {
            return None;
        }
        let file = std::fs::read(path).ok()?;
        if format!("{:x}", Sha256::digest(&file)) != DIGEST.trim() || file.get(..8)? != b"PALVEC01"
        {
            return None;
        }
        let width = read_u32(&file, 8)?;
        let vocab_size = read_u32(&file, 12)?;
        let documents = read_u32(&file, 16)?;
        if width != WIDTH || vocab_size != 100_000 || documents > 30_000 {
            return None;
        }
        let mut sections = Vec::new();
        let mut offset = 44usize;
        for i in 0..6 {
            let end = offset.checked_add(read_u32(&file, 20 + 4 * i)?)?;
            file.get(offset..end)?;
            sections.push(offset..end);
            offset = end;
        }
        if offset != file.len()
            || sections[1].len() != vocab_size * 4
            || sections[2].len() != vocab_size * width
            || sections[4].len() != documents * 4
            || sections[5].len() != documents * width
        {
            return None;
        }
        let vocab: HashMap<_, _> = std::str::from_utf8(&file[sections[0].clone()])
            .ok()?
            .split('\n')
            .enumerate()
            .map(|(id, w)| (w.to_owned(), id))
            .collect();
        let keys: Vec<_> = std::str::from_utf8(&file[sections[3].clone()])
            .ok()?
            .split('\n')
            .map(str::to_owned)
            .collect();
        if vocab.len() != vocab_size || keys.len() != documents {
            return None;
        }
        Some(Self {
            file,
            vocab,
            keys,
            scales: sections[1].clone(),
            tokens: sections[2].clone(),
            doc_scales: sections[4].clone(),
            docs: sections[5].clone(),
        })
    }
    fn encode(&self, question: &str) -> Option<[f32; WIDTH]> {
        let mut out = [0.; WIDTH];
        let folded: String = question
            .chars()
            .take(1024)
            .collect::<String>()
            .nfd()
            .flat_map(char::to_lowercase)
            .filter(|c| !is_combining_mark(*c))
            .collect();
        let mut words = Vec::new();
        let mut latin = String::new();
        for c in folded.chars().chain(std::iter::once(' ')) {
            if c.is_ascii_alphanumeric() {
                latin.push(c);
                continue;
            }
            if !latin.is_empty() {
                words.push(std::mem::take(&mut latin));
            }
            if !c.is_whitespace() && !c.is_control() {
                words.push(c.to_string());
            }
        }
        let mut total = 0;
        for word in words {
            let mut ids = Vec::new();
            let mut at = 0;
            while at < word.len() {
                let rest = &word[at..];
                let found = rest
                    .char_indices()
                    .map(|(a, c)| a + c.len_utf8())
                    .rev()
                    .find_map(|n| {
                        let part = &rest[..n];
                        let id = if at == 0 {
                            self.vocab.get(part)
                        } else {
                            self.vocab.get(&format!("##{part}"))
                        };
                        id.map(|id| (*id, n))
                    });
                let Some((id, bytes)) = found else {
                    ids.clear();
                    break;
                };
                ids.push(id);
                at += bytes;
            }
            for id in ids {
                let k = scale(&self.file, self.scales.start + 4 * id);
                let start = self.tokens.start + WIDTH * id;
                for (v, &byte) in out.iter_mut().zip(&self.file[start..start + WIDTH]) {
                    *v += f32::from(byte as i8) * k;
                }
                total += 1;
            }
        }
        let norm = out.iter().map(|v| v * v).sum::<f32>().sqrt();
        if total == 0 || norm <= 0. {
            return None;
        }
        for v in &mut out {
            *v /= norm;
        }
        Some(out)
    }
    fn ranked<'a>(&'a self, question: &str, eligible: impl Fn(&str) -> bool) -> Vec<&'a str> {
        let Some(v) = self.encode(question) else {
            return Vec::new();
        };
        let mut scores = Vec::new();
        for (i, key) in self.keys.iter().enumerate().filter(|(_, k)| eligible(k)) {
            let start = self.docs.start + i * WIDTH;
            let dot: f32 = self.file[start..start + WIDTH]
                .iter()
                .zip(&v)
                .map(|(&b, &q)| f32::from(b as i8) * q)
                .sum();
            let similarity = dot * scale(&self.file, self.doc_scales.start + 4 * i);
            if similarity >= 0.18 {
                scores.push((key.as_str(), similarity));
            }
        }
        scores.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        scores.into_iter().take(8).map(|(k, _)| k).collect()
    }
    pub(super) fn prose(&self, question: &str) -> Vec<&str> {
        self.ranked(question, |k| k.starts_with("a:") || k.starts_with("g:"))
    }
}
pub(super) fn index(d: &Data) -> Option<&Index> {
    let catalog = d.catalog_text.get_or_init(|| CatalogText::build(d));
    catalog
        .semantic
        .get_or_init(|| Index::load(&d.root))
        .as_ref()
}
pub(super) use relation::lookup as relation_evidence;
