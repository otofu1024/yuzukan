//! rewriter のデータファイル (YAML) をビルド時に静的テーブルへ変換する。
//!
//! 生成物は `OUT_DIR` の `*_table.rs` で、`src/rewriter/` の各モジュールが
//! `include!` する。実行時の YAML パースやテーブル構築は発生しない。
//!
//! `binary_search_by_key` で引くテーブルは `BTreeMap` の反復順で書き出すため、
//! キーは `str` の `Ord` で厳密に昇順 (重複なし) になる。文字列は `{:?}` で
//! Rust のリテラルにするので、引用符、バックスラッシュ、結合文字などは
//! 安全にエスケープされる。

use std::collections::BTreeMap;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::de::DeserializeOwned;

#[derive(Deserialize)]
struct EmojiEntry {
    char: String,
    #[serde(default)]
    readings: Vec<String>,
    #[serde(default)]
    triggers: Vec<String>,
}

#[derive(Deserialize)]
struct EmojiFile {
    #[serde(default)]
    descriptions: BTreeMap<String, String>,
    #[serde(default)]
    entries: Vec<EmojiEntry>,
}

#[derive(Deserialize)]
struct SingleKanjiEntry {
    char: String,
    #[serde(default)]
    readings: Vec<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct SingleKanjiFile {
    #[serde(default)]
    entries: Vec<SingleKanjiEntry>,
}

#[derive(Deserialize)]
struct VariantEntry {
    key: String,
    chain: Vec<String>,
}

#[derive(Deserialize)]
struct SymbolEntry {
    char: String,
    readings: Vec<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct SymbolFile {
    descriptions: BTreeMap<String, String>,
    variants: Vec<VariantEntry>,
    entries: Vec<SymbolEntry>,
}

/// 読み → (記号, 注記) の一覧。
type SymbolBuckets = BTreeMap<String, Vec<(String, Option<String>)>>;

fn main() {
    emoji_table();
    single_kanji_table();
    symbols_table();
}

fn emoji_table() {
    let file: EmojiFile = load("data/emoji.yml");

    // 読みごとの絵文字はファイル順で、同じ読みの中では重複させない。
    let mut by_reading: BTreeMap<String, Vec<String>> = BTreeMap::new();
    // トリガーは porter の出力順 (手動エイリアス → CLDR → ローマ字) を保つ。
    // 同点の候補はこの順で並ぶ。
    let mut triggers: Vec<(String, String)> = Vec::new();
    for entry in file.entries {
        for reading in entry.readings {
            push_unique(by_reading.entry(reading).or_default(), &entry.char);
        }
        for trigger in entry.triggers {
            triggers.push((trigger, entry.char.clone()));
        }
    }

    write_tables(
        "emoji_table.rs",
        &[
            slice(
                "EMOJI_DESCRIPTIONS",
                "(&str, &str)",
                file.descriptions.iter().map(literal),
            ),
            slice(
                "EMOJI_BY_READING",
                "(&str, &[&str])",
                by_reading.iter().map(bucket_literal),
            ),
            slice(
                "EMOJI_TRIGGERS",
                "(&str, &str)",
                triggers.iter().map(literal),
            ),
        ],
    );
}

fn single_kanji_table() {
    let file: SingleKanjiFile = load("data/single_kanji.yml");

    // 同じ漢字が複数回あれば読みを統合し、注記は最初のものを採用する。
    let mut by_reading: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut descriptions: BTreeMap<String, String> = BTreeMap::new();
    for entry in file.entries {
        for reading in entry.readings {
            push_unique(by_reading.entry(reading).or_default(), &entry.char);
        }
        if let Some(description) = entry.description {
            descriptions.entry(entry.char).or_insert(description);
        }
    }

    write_tables(
        "single_kanji_table.rs",
        &[
            slice(
                "SINGLE_KANJI_BY_READING",
                "(&str, &[&str])",
                by_reading.iter().map(bucket_literal),
            ),
            slice(
                "SINGLE_KANJI_DESCRIPTIONS",
                "(&str, &str)",
                descriptions.iter().map(literal),
            ),
        ],
    );
}

fn symbols_table() {
    let file: SymbolFile = load("data/symbols.yml");

    let variants: BTreeMap<String, Vec<String>> = file
        .variants
        .into_iter()
        .map(|entry| (entry.key, entry.chain))
        .collect();

    // 注記は `entries` (mozc 由来、先勝ち) を土台にして、手書きの
    // `descriptions` で上書きする。
    let mut descriptions: BTreeMap<String, String> = BTreeMap::new();
    let mut by_reading = SymbolBuckets::new();
    for entry in file.entries {
        if let Some(description) = &entry.description {
            descriptions
                .entry(entry.char.clone())
                .or_insert_with(|| description.clone());
        }
        for reading in entry.readings {
            let bucket = by_reading.entry(reading).or_default();
            // mozc では品詞違いで同じ読みと記号の組が複数行になることがある。
            if !bucket.iter().any(|(symbol, _)| *symbol == entry.char) {
                bucket.push((entry.char.clone(), entry.description.clone()));
            }
        }
    }
    descriptions.extend(file.descriptions);

    write_tables(
        "symbols_table.rs",
        &[
            slice(
                "SYMBOL_DESCRIPTIONS",
                "(&str, &str)",
                descriptions.iter().map(literal),
            ),
            slice(
                "SYMBOL_VARIANTS",
                "(&str, &[&str])",
                variants.iter().map(bucket_literal),
            ),
            // 型を展開したまま書き出すので、clippy の複雑さの警告は抑止する。
            format!(
                "#[allow(clippy::type_complexity)]\n{}",
                slice(
                    "SYMBOL_BY_READING",
                    "(&str, &[(&str, Option<&str>)])",
                    by_reading.iter().map(bucket_literal),
                )
            ),
        ],
    );
}

/// YAML を読み込み、そのファイルの変更で再実行されるように登録する。
fn load<T: DeserializeOwned>(path: &str) -> T {
    println!("cargo:rerun-if-changed={path}");
    let yaml = fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_yaml::from_str(&yaml).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn push_unique(bucket: &mut Vec<String>, item: &str) {
    if !bucket.iter().any(|existing| existing == item) {
        bucket.push(item.to_string());
    }
}

/// 文字列の組をタプルのリテラルにする。
fn literal(row: impl Debug) -> String {
    format!("{row:?}")
}

/// `(キー, &[値, ...])` のリテラルにする。
fn bucket_literal<T: Debug>((key, values): (&String, &Vec<T>)) -> String {
    format!("({key:?}, &{values:?})")
}

/// `static 名前: &[要素型] = &[...];` を組み立てる。
fn slice(name: &str, element: &str, rows: impl Iterator<Item = String>) -> String {
    let body: String = rows.map(|row| format!("    {row},\n")).collect();
    format!("static {name}: &[{element}] = &[\n{body}];\n")
}

fn write_tables(file_name: &str, tables: &[String]) {
    let out_dir = env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo");
    let path = Path::new(&out_dir).join(file_name);
    fs::write(&path, tables.concat()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}
