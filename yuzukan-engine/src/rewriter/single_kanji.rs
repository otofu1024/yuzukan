//! Single-kanji rewriter: the reading as one kanji at a time (たか → 高,
//! 嵩, 鷹, …), so a character the model and the dictionaries never
//! offer is still reachable. Behind the reading's own symbols and emoji in
//! the chain, since the list is exhaustive.
//!
//! The data is `data/single_kanji.yml`, hand-maintained, one entry per
//! kanji:
//!
//! ```yaml
//! entries:
//!   - char: 髙
//!     readings: [こう, たか]
//!     description: 高の異体字
//! ```
//!
//! For one reading the entries higher in the file come out first. A kanji
//! listed twice is merged (readings from both, the first description
//! wins). The description is the candidate's annotation; [`description`]
//! lets another source's candidate borrow it when it is the same kanji.

use super::{RewriteOutput, Rewriter};

// build.rs が `data/single_kanji.yml` から生成する静的テーブル。
// - `SINGLE_KANJI_BY_READING`: 読み → 漢字一覧 (キー昇順、一覧はファイル順)
// - `SINGLE_KANJI_DESCRIPTIONS`: 漢字 → 注記 (キー昇順)
include!(concat!(env!("OUT_DIR"), "/single_kanji_table.rs"));

/// キー昇順のテーブルを二分探索する。
fn lookup<V: Copy>(table: &[(&str, V)], key: &str) -> Option<V> {
    let index = table.binary_search_by_key(&key, |&(k, _)| k).ok()?;
    Some(table[index].1)
}

/// Annotation for a kanji (`髙` → `高の異体字`), if the data has one.
pub fn description(text: &str) -> Option<&'static str> {
    lookup(SINGLE_KANJI_DESCRIPTIONS, text)
}

/// Rewriter emitting every single kanji for the typed reading.
#[derive(Default)]
pub struct SingleKanjiRewriter;

impl Rewriter for SingleKanjiRewriter {
    fn name(&self) -> &'static str {
        "single_kanji"
    }

    fn rewrite(&self, candidate: &str) -> Vec<RewriteOutput> {
        // 読みの完全一致で、各漢字に注記を添える。
        lookup(SINGLE_KANJI_BY_READING, candidate)
            .into_iter()
            .flatten()
            .map(|&kanji| (kanji.to_string(), description(kanji).map(str::to_string)))
            .collect()
    }

    fn is_catalog(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::rewriter::test_util::{desc, texts};

    #[test]
    fn reading_lists_its_kanji() {
        let out = texts(&SingleKanjiRewriter.rewrite("たか"));
        assert!(out.len() > 10);
        for kanji in ["高", "鷹", "孝", "嵩"] {
            assert!(out.contains(&kanji.to_string()), "{kanji} missing");
        }
        assert_eq!(texts(&SingleKanjiRewriter.rewrite("あいだ")), ["間"]);
        assert!(SingleKanjiRewriter.rewrite("あいだに").is_empty());
    }

    #[test]
    fn variant_carries_its_note() {
        let out = SingleKanjiRewriter.rewrite("あ");
        assert_eq!(desc(&out, "亜"), None);
        assert_eq!(desc(&out, "亞").as_deref(), Some("亜の旧字体"));
        let taka = SingleKanjiRewriter.rewrite("たか");
        assert_eq!(desc(&taka, "髙").as_deref(), Some("高の異体字"));
        assert_eq!(description("髙"), Some("高の異体字"));
        assert_eq!(description("高"), None);
    }

    #[test]
    fn tables_are_sorted_for_binary_search() {
        assert!(SINGLE_KANJI_BY_READING.is_sorted_by(|a, b| a.0 < b.0));
        assert!(SINGLE_KANJI_DESCRIPTIONS.is_sorted_by(|a, b| a.0 < b.0));
    }

    // 以下は build.rs の統合・順序規則を、YAML を介さず小さな入力で確かめる
    // ためのテスト用ヘルパー。規則を変える場合は build.rs と合わせること。
    struct SingleKanjiEntry {
        char: &'static str,
        readings: &'static [&'static str],
        description: Option<&'static str>,
    }

    fn entry(
        char: &'static str,
        readings: &'static [&'static str],
        description: Option<&'static str>,
    ) -> SingleKanjiEntry {
        SingleKanjiEntry {
            char,
            readings,
            description,
        }
    }

    #[derive(Default)]
    struct Table {
        /// 読み → 漢字 (入力順)
        by_reading: HashMap<&'static str, Vec<&'static str>>,
        /// 漢字 → 注記
        descriptions: HashMap<&'static str, &'static str>,
    }

    impl Table {
        fn new(entries: Vec<SingleKanjiEntry>) -> Self {
            let mut table = Table::default();
            for entry in entries {
                for &reading in entry.readings {
                    let kanji = table.by_reading.entry(reading).or_default();
                    if !kanji.contains(&entry.char) {
                        kanji.push(entry.char);
                    }
                }
                if let Some(description) = entry.description {
                    table.descriptions.entry(entry.char).or_insert(description);
                }
            }
            table
        }

        fn rewrite(&self, reading: &str) -> Vec<RewriteOutput> {
            self.by_reading
                .get(reading)
                .into_iter()
                .flatten()
                .map(|&kanji| {
                    let description = self.descriptions.get(kanji).map(|d| d.to_string());
                    (kanji.to_string(), description)
                })
                .collect()
        }
    }

    #[test]
    fn file_order_is_candidate_order() {
        let t = Table::new(vec![
            entry("乙", &["あ"], None),
            entry("甲", &["あ", "こう"], None),
            entry("丙", &["あ"], None),
        ]);
        assert_eq!(texts(&t.rewrite("あ")), ["乙", "甲", "丙"]);
        assert_eq!(texts(&t.rewrite("こう")), ["甲"]);
    }

    #[test]
    fn duplicate_entries_merge() {
        let t = Table::new(vec![
            entry("高", &["こう", "こう"], None),
            entry("髙", &["こう"], None),
            entry("高", &["たか"], Some("旧字体ではない")),
            entry("髙", &[], Some("高の異体字")),
        ]);
        assert_eq!(texts(&t.rewrite("こう")), ["高", "髙"]);
        assert_eq!(texts(&t.rewrite("たか")), ["高"]);
        assert_eq!(
            desc(&t.rewrite("こう"), "高").as_deref(),
            Some("旧字体ではない")
        );
        assert_eq!(t.descriptions["髙"], "高の異体字");
    }
}
