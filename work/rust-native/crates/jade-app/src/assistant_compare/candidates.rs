use super::{Data, has, text};
use crate::assistant_breeding::species_mentions;

fn list_join(s: &str) -> bool {
    matches!(s.trim(), "和"|"与"|"、"|"及"|"或"|"and"|"or"|"and my"|"or my"|"&"|"/")
}

fn group_end(q: &str, mentions: &[(usize, usize, usize)], start: usize) -> usize {
    let mut end = start + 1;
    while end < mentions.len() && list_join(&q[mentions[end-1].2..mentions[end].1]) {
        end += 1;
    }
    // A comma may introduce a separate clause ("except A, B and C are my picks").
    // Extend an English list only when its last name ends the clause or shares the predicate.
    let mut comma_end = end;
    while comma_end < mentions.len() {
        let (_, start, stop) = mentions[comma_end];
        let separator = q[mentions[comma_end-1].2..start].trim();
        if !q[start..stop].is_ascii()
            || !(list_join(separator) || matches!(separator, "," | ", and" | ", or"))
        {
            break;
        }
        comma_end += 1;
    }
    let tail = q[mentions[comma_end-1].2..].trim_start();
    if tail.is_empty() || tail.starts_with([',', '.', ';', '?', '!']) || after_unavailable(tail) {
        comma_end
    } else {
        end
    }
}

fn before_unavailable(s: &str) -> bool {
    if ["不排除", "不用排除", "别排除", "已经不是没有"].iter().any(|word|s.ends_with(word)) {return false;}
    ["不考虑", "排除", "除了", "不用", "不要用", "别用", "不要", "别推荐", "没解锁", "未解锁", "没抓到", "没有", "还没拿到",
     "暂时抓不到", "解锁不了", "不能用", "不能选", "不可用", "不包含", "except", "apart from", "excluding",
     "without", "besides", "other than", "can't get", "cannot get", "don't have", "do not have",
     "haven't unlocked", "haven't got", "can't unlock", "can't use", "cannot use", "rule out", "can’t get", "don’t have", "no"]
        .iter().any(|marker| s.ends_with(marker))
}

fn after_unavailable(s: &str) -> bool {
    let s = s.trim_start_matches(|c:char| c.is_whitespace() || matches!(c,'（'|'('));
    ["还没解锁","我还没解锁","未解锁","没解锁","还没抓到","暂时抓不到","后期才能","要到后期才","解锁不了","还没有","抓不到","用不了","暂时不能用","暂时没有","先不考虑","除外",
     "is end game locked","is end-game locked","is endgame locked","is locked","are locked","is unavailable",
     "isn't available","is not available","are not available","are unavailable","are both locked","is out of reach","is off limits","is off-limits","is off the table"]
        .iter().any(|marker| s.starts_with(marker))
}

pub(super) fn has_exclusion(q: &str) -> bool {
    has(q,&["不考虑","排除","除了","不用","不要","别用","别推荐","解锁","没抓","没有","没拿到","抓不到","不能用","不能选","用不了","不可用","不包含","后期才","除外",
        "locked","unavailable","isn't available","is not available","out of reach","off limits","off-limits","off the table","except","excluding","without","besides","apart from","other than","can't","cannot","don't have","do not have","haven't got","rule out","can’t","don’t","no "])
}

pub(super) fn participants(data: &Data, q: &str) -> (Vec<String>, Vec<String>) {
    let (mentions, _) = species_mentions(&data.species, q);
    let mut considered = Vec::new();
    let mut excluded = Vec::new();
    let mut i=0;
    while i < mentions.len() {
        let j = group_end(q, &mentions, i);
        let before=q[..mentions[i].1].trim_end();
        let before=["任何","这只","我的"," my"," a"," an"," the"].iter()
            .find_map(|p|before.strip_suffix(p)).unwrap_or(before).trim_end();
        let blocked = before_unavailable(before) || after_unavailable(&q[mentions[j-1].2..]);
        let destination=if blocked { &mut excluded } else { &mut considered };
        for &(id,_,_) in &mentions[i..j] {
            let key=text(&data.species[id],"key").to_owned();
            if !destination.contains(&key) {destination.push(key);}
        }
        i=j;
    }
    // Keep the positive set even if every named option is vetoed: an empty positive
    // set means compare the whole handbook. The ranking applies the vetoes first.
    (considered,excluded)
}

pub(super) fn other_task(q: &str) -> bool {
    let title=q.split(['?', '？','\n']).next().unwrap_or(q);
    let unrelated = [
        "配偶","孵出","选亲本","出生","建筑","房顶","墙壁","帕鲁终端","课本","教本","孵蛋器","设置","帕鲁蛋","搬家",
        "掉帧","服务器","控制器","手柄","联机","存档","错误代码","闪退","快捷键","按键","帕鲁箱怎么",
        "孵化出来","已经孵","how to level", "how do i level", "how does breeding", "how to breed",
        "setting","controller","keybind", "game crash","server","bug report","palbox placement","crafting recipe",
    ];
    has(title,&unrelated) || has(title,&["最好怎么","最好如何","用什么办法","怎么才能","怎么办","有什么办法","best way to","best method to"])
}
