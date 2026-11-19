use crate::assistant_data::chinese_count;

/// Byte offsets in the field-normalized player question; callers resolve units and roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Quantity {
    pub start: usize,
    pub end: usize,
    pub value: u64,
}

fn han_digit(c: char) -> bool {
    "零一二两三四五六七八九十百千万".contains(c)
}

fn han_amount(s: &str) -> Option<u64> {
    if s.starts_with('万') { return None; }
    let n = chinese_count(s)?;
    let mut chars = s.chars().rev();
    let digit = chars.next().and_then(|c| chinese_count(&c.to_string())).filter(|&v| (1..10).contains(&v));
    let place = match chars.next() { Some('万') => 1_000, Some('千') => 100, Some('百') => 10, _ => 1 };
    Some(n + digit.unwrap_or(0) * (place - 1))
}

// Mixed place notation such as 1万2千 is one expression. Consume its entire span
// even if the order or the place of the last digit is ambiguous.
fn mixed_amount(s: &str) -> Option<(usize, Option<u64>)> {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if !(c.is_ascii_digit() || han_digit(c)
            || matches!(c, '.' | ',') && s[i + 1..].starts_with(|next: char| next.is_ascii_digit()))
        {
            break;
        }
        end = i + c.len_utf8();
    }
    let token = &s[..end];
    let (first, symbol) = token.char_indices().find(|(_, c)| han_digit(*c))?;
    if !token.bytes().any(|b| b.is_ascii_digit()) ||
        (matches!(symbol, '万' | '千' | '百' | '十') && first + symbol.len_utf8() == end) {
        return None; // A simple suffix, e.g. 2.5万, belongs to ascii_amount.
    }
    let parse = || {
        let mut total = 0u64;
        let mut previous = u64::MAX;
        let mut start = 0;
        for (i, c) in token.char_indices() {
            let place = match c { '万' => 10_000, '千' => 1_000, '百' => 100, '十' => 10, _ => continue };
            let part = &token[start..i];
            let n = part.parse::<u64>().ok().or_else(|| chinese_count(part))?;
            if n == 0 || place >= previous { return None; }
            let term = n.checked_mul(place)?;
            if term >= previous { return None; }
            total = total.checked_add(term)?;
            start = i + c.len_utf8();
            previous = place;
        }
        if start == 0 || start != token.len() { return None; }
        Some(total)
    };
    Some((end, parse()))
}

fn fractional_component(before: &str, after: &str, allow_meter: bool) -> bool {
    let numeric = |c: char| c.is_ascii_digit() || han_digit(c);
    let before = before.trim_end();
    let after = after.trim_start();
    for mark in ["/", "⁄", "／", "分之"] {
        if allow_meter && mark != "分之" { continue; }
        if after.strip_prefix(mark).is_some_and(|s| s.trim_start().starts_with(numeric))
            || before.strip_suffix(mark).is_some_and(|s| s.trim_end().ends_with(numeric)) {
            return true;
        }
    }
    false
}

fn ascii_amount(s: &str) -> (usize, Option<u64>) {
    let b = s.as_bytes();
    let mut end = 0;
    let mut group = 0;
    let mut grouped = false;
    let mut valid = true;
    let mut numerator = Some(0u128);
    while let Some(&c) = b.get(end) {
        if c.is_ascii_digit() {
            numerator = numerator.and_then(|n| n.checked_mul(10)?.checked_add(u128::from(c - b'0')));
            group += 1;
        } else if c == b',' && b.get(end + 1).is_some_and(u8::is_ascii_digit) {
            valid &= if grouped { group == 3 } else { (1..=3).contains(&group) };
            group = 0;
            grouped = true;
        } else { break; }
        end += 1;
    }
    valid &= !grouped || group == 3;
    let decimal = b.get(end) == Some(&b'.') && b.get(end + 1).is_some_and(u8::is_ascii_digit);
    let mut denominator = Some(1u128);
    if decimal {
        end += 1;
        while let Some(c) = b.get(end).filter(|c| c.is_ascii_digit()) {
            numerator = numerator.and_then(|n| n.checked_mul(10)?.checked_add(u128::from(*c - b'0')));
            denominator = denominator.and_then(|n| n.checked_mul(10));
            end += 1;
        }
    }
    let tail = &s[end..];
    let scale = match tail.chars().next() {
        Some('万') => { end += '万'.len_utf8(); 10_000 }
        Some('千') => { end += '千'.len_utf8(); 1_000 }
        Some('百') => { end += '百'.len_utf8(); 100 }
        Some('十') => { end += '十'.len_utf8(); 10 }
        Some('k' | 'K') if !tail[1..].starts_with(|c: char| c.is_ascii_alphabetic()) => { end += 1; 1_000 }
        _ => ["million", "thousand", "hundred"].into_iter().zip([1_000_000, 1_000, 100])
            .find_map(|(word, scale)| {
                let rest = tail.trim_start().strip_prefix(word)?;
                if rest.starts_with(|c: char| c.is_ascii_alphabetic()) { return None; }
                end += tail.len() - rest.len();
                Some(scale)
            }).unwrap_or(1),
    };
    let value = numerator.zip(denominator).and_then(|(n, d)| {
        let scaled = n.checked_mul(scale)?;
        if !valid || (decimal && scale == 1) || scaled % d != 0 { return None; }
        u64::try_from(scaled / d).ok()
    });
    (end, value)
}

fn under_hundred(s: &str) -> Option<(usize, u64)> {
    const SMALL: &[&str] = &["zero", "one", "two", "three", "four", "five", "six", "seven", "eight",
        "nine", "ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen",
        "seventeen", "eighteen", "nineteen"];
    const TENS: &[&str] = &["twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
    let first = s.split(|c: char| !c.is_ascii_alphabetic()).next()?;
    if let Some(value) = SMALL.iter().position(|&w| w == first) { return Some((first.len(), value as u64)); }
    let value = (TENS.iter().position(|&w| w == first)? as u64 + 2) * 10;
    let tail = s[first.len()..].trim_start_matches([' ', '-']);
    let unit = tail.split(|c: char| !c.is_ascii_alphabetic()).next()?;
    if let Some(n) = SMALL[1..10].iter().position(|&w| w == unit) {
        return Some((s.len() - tail.len() + unit.len(), value + n as u64 + 1));
    }
    Some((first.len(), value))
}

fn next_text(s: &str) -> &str {
    s.trim_start_matches([' ', '-'])
}

fn scale_word<'a>(s: &'a str, word: &str) -> Option<&'a str> {
    let rest = next_text(s).strip_prefix(word)?;
    (!rest.starts_with(|c: char| c.is_ascii_alphabetic())).then_some(rest)
}

fn following_number(s: &str) -> &str {
    next_text(scale_word(s, "and").unwrap_or(s))
}

fn under_thousand(s: &str) -> Option<(usize, u64)> {
    let (end, first) = under_hundred(s)?;
    let Some(rest) = scale_word(&s[end..], "hundred").filter(|_| (1..10).contains(&first)) else {
        return Some((end, first));
    };
    let remaining = following_number(rest);
    let (end, extra) = under_hundred(remaining)
        .map(|(n, v)| (s.len() - remaining.len() + n, v))
        .unwrap_or((s.len() - rest.len(), 0));
    Some((end, first * 100 + extra))
}

fn english_number(s: &str) -> Option<(usize, u64)> {
    let (mut end, mut group) = under_thousand(s)?;
    let mut total = 0;
    for (label, scale) in [("million", 1_000_000), ("thousand", 1_000)] {
        if let Some(rest) = scale_word(&s[end..], label) {
            total += group * scale;
            let next = following_number(rest);
            (end, group) = under_thousand(next)
                .map(|(n, v)| (s.len() - next.len() + n, v))
                .unwrap_or((s.len() - rest.len(), 0));
        }
    }
    Some((end, total + group))
}

pub(crate) fn integers(q: &str) -> Vec<Quantity> {
    scan(q, false)
}

/// Condenser progress has two integer fields separated by a slash (e.g. 3/12).
pub(super) fn meter_integers(q: &str) -> Vec<Quantity> {
    scan(q, true)
}

fn scan(q: &str, allow_meter: bool) -> Vec<Quantity> {
    let mut found = Vec::new();
    let mut start = 0;
    while start < q.len() {
        let tail = &q[start..];
        let first = tail.chars().next().unwrap();
        let (len, value) = if (first.is_ascii_digit() || han_digit(first))
            && let Some(mixed) = mixed_amount(tail) {
            mixed
        } else if first.is_ascii_digit() {
            ascii_amount(tail)
        } else if han_digit(first) {
            let len = tail.find(|c| !han_digit(c)).unwrap_or(tail.len());
            (len, han_amount(&tail[..len]))
        } else if first.is_ascii_alphabetic() {
            let len = tail.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(tail.len());
            english_number(tail).map(|(n, v)| (n, Some(v))).unwrap_or((len, None))
        } else { (first.len_utf8(), None) };
        let end = start + len;
        if let Some(value) = value {
            let before = &q[..start];
            let after = &q[end..];
            let preceding_word = before.trim_end().rsplit(|c: char| !c.is_ascii_alphabetic()).next();
            let negative = before.trim_end().ends_with(['-', '−', '负']) || matches!(preceding_word,Some("minus" | "negative"));
            let word_fragment = first.is_ascii() && (before.ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
                || after.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_'));
            let fraction = before.ends_with(['.', '点']) && before.chars().rev().nth(1).is_none_or(|c| c.is_ascii_digit() || han_digit(c))
                || after.strip_prefix('点').is_some_and(|s| s.starts_with(han_digit))
                || fractional_component(before, after, allow_meter);
            if !negative && !word_fragment && !fraction {
                found.push(Quantity { start, end, value });
            }
        }
        start = end;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_keep_exact_spans_and_do_not_round_invalid_amounts() {
        let q = "用2.5万或1,950金币买十二瓶，twenty-three copies，3k coins，0个";
        let spans: Vec<_> = integers(q).into_iter().map(|n| (&q[n.start..n.end], n.value)).collect();
        assert_eq!(spans, [("2.5万",25000),("1,950",1950),("十二",12),("twenty-three",23),("3k",3000),("0",0)]);
        let q = "one thousand nine hundred and fifty coins, 两千五金币、两千零五金币、一百二十五只, 1.5 million gold";
        assert_eq!(integers(q).iter().map(|n| n.value).collect::<Vec<_>>(), [1950, 2500, 2005, 125, 1500000]);
        for s in ["-16只", "v1.0.5", "2.7瓶", "1,95个", "X52", "18446744073709551616个", "三点五只"] {
            assert!(integers(s).is_empty(), "{s}");
        }
    }

    #[test]
    fn mixed_places_negative_words_and_fractions_leave_no_partial_count() {
        let q = "预算1万2千金币，备用三万4千，共有12 bottles";
        let spans: Vec<_> = integers(q).into_iter().map(|n| (&q[n.start..n.end], n.value)).collect();
        assert_eq!(spans,[("1万2千",12000),("三万4千",34000),("12",12)]);
        for q in ["1万2金币","1万2万个","1.5万2千","negative twelve bottles","minus  12","1/2瓶","1 / 2 瓶","三分之一瓶"] {
            assert!(integers(q).is_empty(), "{q}: {:?}",integers(q));
        }
        assert_eq!(integers("部分为1/2瓶，另买3瓶").iter().map(|n|n.value).collect::<Vec<_>>(),[3]);
        assert_eq!(meter_integers("进度3/12").iter().map(|n|n.value).collect::<Vec<_>>(),[3,12]);
        assert!(meter_integers("百分之一、negative twelve、1万2").is_empty());
    }
}
