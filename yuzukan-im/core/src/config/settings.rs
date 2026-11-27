//! Settings configuration
//!
//! Manages user-configurable settings for the IME.
//! Default values are defined in `config/default.toml`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use yuzukan_engine::{
    BracketStyle, DateConfig, ModelSource, PunctuationStyle, SlashStyle, SymbolStyle, WidthRules,
};

/// Default configuration TOML embedded from config/default.toml
const DEFAULT_CONFIG_TOML: &str = include_str!("../../config/default.toml");

/// Configuration settings for the IME
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Conversion settings
    pub conversion: ConversionSettings,
    /// Learning cache settings
    pub learning: LearningSettings,
    /// What the aux line shows
    pub display: DisplaySettings,
    /// Which symbol each configurable key types
    pub symbol: SymbolSettings,
    /// The width kana input comes out at, per character group
    pub width: WidthRules,
    /// Date/time phrases and their formats
    pub date: DateConfig,
    /// Conversion models, keyed by the name `model` / `light_model` refer to
    pub models: BTreeMap<String, ModelDef>,
}

/// One `[models]` entry as written: a string (the path of a local GGUF,
/// `tokenizer.json` beside it) or a table `{ repo, filename }` (a HuggingFace
/// file, `tokenizer.json` in the same repo). Kept as TOML so a broken entry
/// fails only when used, naming its own keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelDef(toml::Value);

/// The table form of a `[models]` entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HuggingFaceModel {
    repo: String,
    filename: String,
}

impl ModelDef {
    fn source(&self) -> Result<ModelSource> {
        match &self.0 {
            toml::Value::String(path) => Ok(ModelSource::Path(PathBuf::from(path))),
            toml::Value::Table(_) => {
                let HuggingFaceModel { repo, filename } = self.0.clone().try_into()?;
                Ok(ModelSource::HuggingFace { repo, filename })
            }
            other => anyhow::bail!(
                "expected a path string or {{ repo, filename }}, got {}",
                other.type_str()
            ),
        }
    }
}

/// The space the Space key inputs while typing kana. Alphabet and emoji
/// input always take the ASCII one, like the width rules leave direct input
/// alone.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpaceStyle {
    /// The ASCII space.
    #[default]
    Half,
    /// The ideographic space `　`.
    Full,
}

/// Which symbol the keys with more than one conventional output type. The
/// width these settle at is `[width]`, not this section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolSettings {
    /// What `,` and `.` type: 、。 ，． 、． ，。
    pub punctuation: PunctuationStyle,
    /// What `[` and `]` type: 「」 or []
    pub bracket: BracketStyle,
    /// What `/` types: ・ or /
    pub slash: SlashStyle,
    /// The space Space inputs while typing kana
    pub space: SpaceStyle,
}

impl SymbolSettings {
    /// The key-to-symbol style the romaji converter is built with. Space is
    /// not part of it: no romaji rule types a space.
    pub fn style(&self) -> SymbolStyle {
        SymbolStyle {
            punctuation: self.punctuation,
            bracket: self.bracket,
            slash: self.slash,
        }
    }
}

/// When the candidate window opens.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateWindow {
    /// While typing too, then through the conversion.
    #[default]
    Always,
    /// Only once Space starts a conversion.
    Conversion,
}

/// Color theme for the candidate window.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorTheme {
    /// Neutral monochrome auto Light/Dark palette.
    #[default]
    #[serde(alias = "monochrome")]
    System,
    /// Auto Light/Dark palette centered on Cream White, Charcoal Brown, and Yuzu Yellow.
    Yuzu,
    /// Always use the light Yuzu palette regardless of macOS appearance.
    #[serde(alias = "yuzu_light")]
    YuzuLight,
    /// Always use the dark Yuzu palette regardless of macOS appearance.
    #[serde(alias = "yuzu_dark")]
    YuzuDark,
    /// Auto Light/Dark palette accented with Yuzu Green.
    #[serde(alias = "yuzu_leaf", alias = "yuzu-green", alias = "yuzu_green")]
    YuzuLeaf,
}

/// Aux-line and candidate window display settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySettings {
    /// Start with the detailed aux line (Ctrl+Shift+V toggles it live):
    /// which part the alternatives cover, inference timing, the model that
    /// ran, and the context handed to it.
    pub verbose: bool,
    /// When the candidate window opens: while typing too, or only once
    /// Space starts a conversion.
    pub candidate_window: CandidateWindow,
    /// Color theme for the candidate window.
    #[serde(default)]
    pub theme: ColorTheme,
}

/// Conversion strategy mode
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrategyMode {
    /// Adaptive: dynamically switch between main and light models based on latency
    #[default]
    Adaptive,
    /// Light: use light_model only (loaded into main slot, beam search on Space)
    Light,
    /// Main: use main model only (no light model loaded)
    Main,
}

/// Conversion-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionSettings {
    /// Conversion strategy mode (adaptive, light, main)
    pub strategy: StrategyMode,
    /// Number of candidates to show on Space conversion
    pub num_candidates: usize,
    /// Use surrounding text (text left of cursor) as context for conversion
    pub use_context: bool,
    /// Maximum number of surrounding text characters passed to the conversion API
    pub context_chars: usize,
    /// Conversion persona: free text about what the user writes (e.g.
    /// 「プログラミング」) prepended to the model's left context on every
    /// conversion so results lean toward it (くろーど → Claude). Last 25
    /// chars are used; empty disables.
    pub persona: String,
    /// Maximum reading length (in characters) converted by the model in a single
    /// call during live conversion. The composing buffer is split into chunks
    /// of at most this many characters so per-keystroke latency stays bounded
    /// for long input; each chunk's left context is the converted text of the
    /// preceding chunks.
    pub chunk_chars: usize,
    /// Marks (、。！？…) a chunk containing Japanese keeps instead of
    /// splitting there, so a sentence keeps converting as one unit.
    pub chunk_symbols: usize,
    /// Digits a chunk containing Japanese keeps. The default 0 keeps them
    /// out of the converter entirely, which is what protects them: the
    /// model hallucinates on digit runs, dropping or duplicating figures.
    /// Raising it lets short runs (「だい3かい」) convert with the text
    /// around them; the digits that fit ride along and the rest open the
    /// next chunk.
    pub chunk_digits: usize,
    /// Alphabet chars a chunk containing Japanese keeps. The default 0 keeps
    /// latin text passthrough, and keeps the in-progress romaji tail out of
    /// the converter: while 「わせだd」 is being typed, the `d` is an unfired
    /// keystroke, not text. Raising it lets 「Rustで」 convert as one unit at
    /// the cost of feeding that tail to the model on every keystroke.
    pub chunk_alphabets: usize,
    /// Path to dictionary binary file (optional, defaults to data_dir/dict.bin)
    pub dict_path: Option<String>,
    /// Main model: a key in `[models]`
    pub model: String,
    /// Beam search model (Space conversion, adaptive downgrade): a key in `[models]`
    pub light_model: String,
    /// Chars the beam covers, snapped to chunk boundaries: the trailing
    /// Japanese chunks fitting this budget, always at least the last one.
    /// A digit/symbol chunk and a manual break both stop the span.
    pub beam_chars: usize,
    /// Beam width: how many alternatives the beam returns
    pub beam_width: usize,
    /// Maximum acceptable latency in milliseconds for auto-suggest (0 = disabled)
    /// When a main model conversion exceeds this, the engine adaptively switches to light_model
    pub max_latency_ms: u64,
    /// Number of threads for llama.cpp inference (0 = all cores, llama.cpp default)
    pub n_threads: u32,
    /// Enable live conversion at startup (Ctrl+Shift+L still toggles at runtime)
    pub live_conversion: bool,
}

/// Learning cache settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSettings {
    /// Whether learning is enabled
    pub enabled: bool,
    /// Maximum number of total entries in the learning cache
    pub max_entries: usize,
    /// Maximum surface length (in characters) recorded into the learning
    /// cache; longer conversion results (e.g. whole live-converted
    /// sentences) are not learned
    pub max_surface_chars: usize,
}

impl Default for Settings {
    fn default() -> Self {
        toml::from_str(DEFAULT_CONFIG_TOML).expect("embedded default.toml must be valid")
    }
}

/// Recursively merge `overlay` TOML values on top of `base`.
fn merge_toml(base: &mut toml::Value, overlay: &toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) => {
            for (key, value) in overlay_table {
                if let Some(base_value) = base_table.get_mut(key) {
                    merge_toml(base_value, value);
                } else {
                    base_table.insert(key.clone(), value.clone());
                }
            }
        }
        (base, _) => {
            *base = overlay.clone();
        }
    }
}

/// Parse user TOML content merged on top of default.toml.
fn parse_with_defaults(user_content: &str) -> Result<Settings> {
    let mut base: toml::Value = toml::from_str(DEFAULT_CONFIG_TOML)?;
    let user: toml::Value = toml::from_str(user_content)?;
    merge_toml(&mut base, &user);
    // A `[models]` entry replaces the default under the same key whole: a
    // partial `{ repo = ... }` must not inherit the default's filename (a
    // plain string already replaces through `merge_toml`).
    if let Some(user_models) = user.get("models").and_then(toml::Value::as_table)
        && let Some(base_models) = base.get_mut("models").and_then(toml::Value::as_table_mut)
    {
        base_models.extend(user_models.clone());
    }
    Ok(base.try_into()?)
}

/// Get the project directories for yuzukan-im.
fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "yuzukan", "yuzukan-im")
}

/// Legacy project directories (`com.karukan.karukan-im`) read as fallback
/// when the yuzukan location does not yet have user config or data.
fn legacy_project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "karukan", "karukan-im")
}

/// `exe` が `Contents/MacOS/` にあり、`Contents/Resources/<relative>` が
/// 存在する場合にそのパスを返す。開発時のビルドでは常に `None`。
fn bundled_file_beside(exe: &Path, relative: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if !macos.ends_with("Contents/MacOS") {
        return None;
    }
    let path = macos.parent()?.join("Resources").join(relative);
    path.is_file().then_some(path)
}

/// HuggingFace のモデルと同じものが `models/<repo>/` に同梱されていれば
/// ローカルファイルとして扱う(ダウンロードが不要になる)。
/// GGUF と `tokenizer.json` の両方がそろっていない場合は元のまま。
fn prefer_bundled(source: ModelSource, bundled: impl Fn(&Path) -> Option<PathBuf>) -> ModelSource {
    if let ModelSource::HuggingFace { repo, filename } = &source {
        let dir = Path::new("models").join(repo);
        if let Some(gguf) = bundled(&dir.join(filename))
            && bundled(&dir.join("tokenizer.json")).is_some()
        {
            return ModelSource::Path(gguf);
        }
    }
    source
}

impl Settings {
    /// Resolve a `[models]` key into a [`ModelSource`].
    pub fn model_source(&self, key: &str) -> Result<ModelSource> {
        let def = self.models.get(key).ok_or_else(|| {
            let known: Vec<&str> = self.models.keys().map(String::as_str).collect();
            anyhow::anyhow!(
                "model '{}' is not defined in [models] (defined: {})",
                key,
                known.join(", ")
            )
        })?;
        let source = def.source().with_context(|| format!("[models.{key}]"))?;
        Ok(prefer_bundled(source, Self::bundled_file))
    }

    /// アプリバンドルの `Contents/Resources/` に同梱されたファイルを返す。
    /// pkg で配布する場合に辞書とモデルがここに入る。
    pub fn bundled_file(relative: &Path) -> Option<PathBuf> {
        bundled_file_beside(&std::env::current_exe().ok()?, relative)
    }

    /// Get the data directory path
    pub fn data_dir() -> Option<PathBuf> {
        project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
    }

    /// Get the legacy data directory path (`com.karukan.karukan-im`)
    pub fn legacy_data_dir() -> Option<PathBuf> {
        legacy_project_dirs().map(|dirs| dirs.data_dir().to_path_buf())
    }

    /// Get the configuration directory path
    pub fn config_dir() -> Option<PathBuf> {
        project_dirs().map(|dirs| dirs.config_dir().to_path_buf())
    }

    /// Get the configuration file path
    pub fn config_file() -> Option<PathBuf> {
        Self::config_dir().map(|dir| dir.join("config.toml"))
    }

    /// Get the legacy configuration file path (`com.karukan.karukan-im`)
    pub fn legacy_config_file() -> Option<PathBuf> {
        legacy_project_dirs().map(|dirs| dirs.config_dir().join("config.toml"))
    }

    /// Get the user dictionary directory path.
    ///
    /// All files in this directory are automatically loaded as user dictionaries.
    /// Default: `~/.local/share/yuzukan-im/user_dicts/`
    pub fn user_dict_dir() -> Option<PathBuf> {
        Self::data_dir().map(|dir| dir.join("user_dicts"))
    }

    /// Get the legacy user dictionary directory path (`com.karukan.karukan-im`)
    pub fn legacy_user_dict_dir() -> Option<PathBuf> {
        Self::legacy_data_dir().map(|dir| dir.join("user_dicts"))
    }

    /// Get the learning cache file path.
    ///
    /// Default: `~/.local/share/yuzukan-im/learning.tsv`
    pub fn learning_file() -> Option<PathBuf> {
        Self::data_dir().map(|dir| dir.join("learning.tsv"))
    }

    /// Get the legacy learning cache file path (`com.karukan.karukan-im`)
    pub fn legacy_learning_file() -> Option<PathBuf> {
        Self::legacy_data_dir().map(|dir| dir.join("learning.tsv"))
    }

    /// Load settings from the default configuration file, falling back to
    /// the legacy karukan path if present, or embedded default.toml.
    pub fn load() -> Result<Self> {
        let Some(config_file) = Self::config_file() else {
            warn!("Could not determine config directory, using defaults");
            return Ok(Self::default());
        };

        if config_file.exists() {
            debug!("Loading config from {:?}", config_file);
            return Self::load_from(&config_file);
        }

        if let Some(legacy) = Self::legacy_config_file()
            && legacy.exists()
        {
            debug!("Loading config from legacy path {:?}", legacy);
            return Self::load_from(&legacy);
        }

        debug!("Config file not found, using defaults");
        Ok(Self::default())
    }

    /// `load`, falling back to the defaults when the config cannot be read:
    /// an IME must start. The failure is logged, so a typo never silently
    /// resets every setting.
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_else(|e| {
            error!("config.toml not loaded, using defaults: {e:#}");
            Self::default()
        })
    }

    /// Load settings from a specific file, merged on top of defaults.
    pub fn load_from(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let settings = parse_with_defaults(&content)?;
        // Surface broken model entries now, without failing the load: a key
        // that is never used must not cost the rest of the config.
        for key in settings.models.keys() {
            if let Err(e) = settings.model_source(key) {
                warn!("{e:#}");
            }
        }
        Ok(settings)
    }

    /// Save settings to the default configuration file
    pub fn save(&self) -> Result<()> {
        let Some(config_file) = Self::config_file() else {
            anyhow::bail!("Could not determine config directory");
        };

        debug!("Saving config to {:?}", config_file);
        self.save_to(&config_file)
    }

    /// Save settings to a specific file
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use yuzukan_engine::Width;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.conversion.num_candidates, 9);
        assert!(settings.conversion.use_context);
        assert_eq!(settings.conversion.context_chars, 10);
        assert!(settings.learning.enabled);
        assert_eq!(settings.learning.max_entries, 10000);
        assert_eq!(settings.learning.max_surface_chars, 50);
    }

    #[test]
    fn test_default_symbol_and_width_settings() {
        // Shipped defaults: the Japanese symbols, and kana input that comes
        // out full-width apart from digits.
        let settings = Settings::default();
        assert_eq!(settings.symbol.punctuation, PunctuationStyle::KutenTouten);
        assert_eq!(settings.symbol.bracket, BracketStyle::Corner);
        assert_eq!(settings.symbol.slash, SlashStyle::MiddleDot);
        assert_eq!(settings.symbol.space, SpaceStyle::Half);
        assert_eq!(settings.width.kana_symbol, Width::Full);
        assert_eq!(settings.width.ascii_symbol, Width::Full);
        assert_eq!(settings.width.digit, Width::Half);
    }

    #[test]
    fn test_symbol_style_is_written_as_the_symbols() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[symbol]
punctuation = "，．"
bracket = "[]"
slash = "/"
space = "half"
"#
        )
        .unwrap();

        let settings = Settings::load_from(file.path()).unwrap();
        assert_eq!(settings.symbol.punctuation, PunctuationStyle::CommaPeriod);
        assert_eq!(settings.symbol.bracket, BracketStyle::Square);
        assert_eq!(settings.symbol.slash, SlashStyle::Slash);
        assert_eq!(settings.symbol.space, SpaceStyle::Half);
    }

    #[test]
    fn test_width_partial_config() {
        // Setting one group leaves the others at their default.
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[width]
ascii_symbol = "half"
digit = "full"
"#
        )
        .unwrap();

        let settings = Settings::load_from(file.path()).unwrap();
        assert_eq!(settings.width.ascii_symbol, Width::Half);
        assert_eq!(settings.width.digit, Width::Full);
        assert_eq!(settings.width.kana_symbol, Width::Full);
    }

    #[test]
    fn test_unknown_keys_are_ignored() {
        // A config written for an older version (e.g. the removed
        // short_input_threshold key) must still load: unknown keys are
        // ignored, the renamed setting falls back to its default, and
        // the other overrides keep applying.
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[conversion]
short_input_threshold = 10
beam_width = 5
"#
        )
        .unwrap();

        let settings = Settings::load_from(file.path()).unwrap();
        assert_eq!(settings.conversion.chunk_chars, 30);
        assert_eq!(settings.conversion.beam_width, 5);
    }

    #[test]
    fn test_learning_partial_config() {
        // Overriding one learning key keeps the defaults for the others.
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[learning]
max_surface_chars = 10
"#
        )
        .unwrap();

        let settings = Settings::load_from(file.path()).unwrap();
        assert_eq!(settings.learning.max_surface_chars, 10);
        assert!(settings.learning.enabled);
        assert_eq!(settings.learning.max_entries, 10000);
    }

    #[test]
    fn test_candidate_window_setting() {
        assert_eq!(
            Settings::default().display.candidate_window,
            CandidateWindow::Always
        );
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "[display]\ncandidate_window = \"conversion\"").unwrap();
        let settings = Settings::load_from(file.path()).unwrap();
        assert_eq!(
            settings.display.candidate_window,
            CandidateWindow::Conversion
        );
        assert!(!settings.display.verbose);
    }

    #[test]
    fn test_date_phrases_replace_the_shipped_list() {
        // phrases is an array: writing one replaces the shipped list
        // wholesale (extend by copying default.toml's list). offset_days
        // defaults to 0, and the shared [date] formats stay untouched.
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[date]
phrases = [
    {{ reading = "らいしゅう", offset_days = 7 }},
    {{ reading = "きょう", formats = ["{{YEAR}}年"] }},
]
"#
        )
        .unwrap();

        let settings = Settings::load_from(file.path()).unwrap();
        let phrases = &settings.date.phrases;
        assert_eq!(phrases.len(), 2);
        assert_eq!(phrases[0].reading, "らいしゅう");
        assert_eq!(phrases[0].offset_days, 7);
        assert!(phrases[0].formats.is_none());
        assert_eq!(phrases[1].offset_days, 0);
        assert_eq!(
            phrases[1].formats.as_deref(),
            Some(["{YEAR}年".to_string()].as_slice())
        );
        assert!(!settings.date.formats.is_empty());
    }

    #[test]
    fn test_serialize_deserialize() {
        let settings = Settings::default();
        let toml_str = toml::to_string(&settings).unwrap();
        let loaded: Settings = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            loaded.conversion.num_candidates,
            settings.conversion.num_candidates
        );
    }

    #[test]
    fn test_load_from_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[conversion]
num_candidates = 5
use_context = false
"#
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let settings = Settings::load_from(&path).unwrap();
        assert_eq!(settings.conversion.num_candidates, 5);
        assert!(!settings.conversion.use_context);
    }

    #[test]
    fn test_user_dict_dir() {
        let dir = Settings::user_dict_dir();
        // Should return Some on systems with a home directory
        if let Some(dir) = dir {
            assert!(dir.ends_with("user_dicts"));
        }
    }

    #[test]
    fn test_project_dirs_use_yuzukan() {
        if let Some(data_dir) = Settings::data_dir() {
            assert!(data_dir.to_string_lossy().contains("yuzukan-im"));
        }
        if let Some(config_dir) = Settings::config_dir() {
            assert!(config_dir.to_string_lossy().contains("yuzukan-im"));
        }
    }

    #[test]
    fn test_partial_config() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[conversion]
num_candidates = 3
"#
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let settings = Settings::load_from(&path).unwrap();
        assert_eq!(settings.conversion.num_candidates, 3);
        // Should use default for unspecified values
        assert!(settings.conversion.use_context);
        assert_eq!(settings.conversion.context_chars, 10);
    }

    #[test]
    fn test_default_models_resolve() {
        let settings = Settings::default();
        assert_eq!(settings.conversion.model, "jinen-v2-small-q5");
        assert_eq!(settings.conversion.light_model, "jinen-v2-xsmall-q5");
        let source = settings.model_source(&settings.conversion.model).unwrap();
        assert_eq!(
            source,
            ModelSource::HuggingFace {
                repo: "togatogah/jinen-v2-small.gguf".to_string(),
                filename: "jinen-v2-small-Q5_K_M.gguf".to_string(),
            }
        );
        for key in settings.models.keys() {
            settings.model_source(key).unwrap();
        }
    }

    #[test]
    fn test_user_model_entry_extends_defaults() {
        let settings = parse_with_defaults(
            r#"
[conversion]
model = "my-model"

[models]
my-model = "/home/user/models/my.gguf"
"#,
        )
        .unwrap();
        assert_eq!(
            settings.model_source("my-model").unwrap(),
            ModelSource::Path(PathBuf::from("/home/user/models/my.gguf"))
        );
        // The default entries survive next to the user's.
        settings.model_source("jinen-v2-xsmall-q5").unwrap();
    }

    #[test]
    fn test_user_model_entry_replaces_default_whole() {
        // A user entry under a default key replaces it whole: a partial
        // table must not inherit the default's filename.
        let settings = parse_with_defaults(
            r#"
[models]
jinen-v2-small-q5 = "/home/user/models/my.gguf"
jinen-v2-xsmall-q5 = { repo = "other/repo.gguf" }
"#,
        )
        .unwrap();
        assert_eq!(
            settings.model_source("jinen-v2-small-q5").unwrap(),
            ModelSource::Path(PathBuf::from("/home/user/models/my.gguf"))
        );
        assert!(settings.model_source("jinen-v2-xsmall-q5").is_err());
    }

    #[test]
    fn test_unknown_model_key_lists_defined_keys() {
        let settings = Settings::default();
        let err = settings.model_source("nope").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("'nope'"), "{msg}");
        assert!(msg.contains("jinen-v2-small-q5"), "{msg}");
    }

    #[test]
    fn test_broken_model_entries_name_their_keys() {
        // Broken entries load (the rest of the config survives) and fail
        // only when used, each saying what is wrong with it.
        let settings = parse_with_defaults(
            r#"
[models]
no-filename = { repo = "owner/repo.gguf" }
typo = { repo = "owner/repo.gguf", file_name = "m.gguf" }
number = 42
"#,
        )
        .unwrap();
        let error = |key: &str| format!("{:#}", settings.model_source(key).unwrap_err());
        assert!(
            error("no-filename").contains("[models.no-filename]"),
            "{}",
            error("no-filename")
        );
        assert!(
            error("no-filename").contains("missing field `filename`"),
            "{}",
            error("no-filename")
        );
        assert!(
            error("typo").contains("unknown field `file_name`"),
            "{}",
            error("typo")
        );
        assert!(
            error("number").contains("expected a path string"),
            "{}",
            error("number")
        );
        settings.model_source("jinen-v2-small-q5").unwrap();
    }

    #[test]
    fn test_strategy_default_when_unspecified() {
        // When strategy is not specified, it should default to Adaptive
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[conversion]
num_candidates = 5
"#
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let settings = Settings::load_from(&path).unwrap();
        assert_eq!(settings.conversion.strategy, StrategyMode::Adaptive);
    }

    #[test]
    fn test_strategy_adaptive() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[conversion]
strategy = "adaptive"
"#
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let settings = Settings::load_from(&path).unwrap();
        assert_eq!(settings.conversion.strategy, StrategyMode::Adaptive);
    }

    #[test]
    fn test_strategy_light() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[conversion]
strategy = "light"
"#
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let settings = Settings::load_from(&path).unwrap();
        assert_eq!(settings.conversion.strategy, StrategyMode::Light);
    }

    #[test]
    fn test_strategy_main() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[conversion]
strategy = "main"
"#
        )
        .unwrap();

        let path = file.path().to_path_buf();
        let settings = Settings::load_from(&path).unwrap();
        assert_eq!(settings.conversion.strategy, StrategyMode::Main);
    }

    #[test]
    fn test_bundled_model_replaces_download() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir
            .path()
            .join("yuzukan.app/Contents/MacOS/yuzukan-imserver");
        let models = dir
            .path()
            .join("yuzukan.app/Contents/Resources/models/owner/repo");
        fs::create_dir_all(&models).unwrap();
        fs::write(models.join("model.gguf"), b"gguf").unwrap();
        let source = ModelSource::HuggingFace {
            repo: "owner/repo".to_string(),
            filename: "model.gguf".to_string(),
        };
        let bundled = |relative: &Path| bundled_file_beside(&exe, relative);

        // tokenizer.json が欠けている間はダウンロードのまま
        assert_eq!(prefer_bundled(source.clone(), bundled), source);

        fs::write(models.join("tokenizer.json"), b"{}").unwrap();
        assert_eq!(
            prefer_bundled(source.clone(), bundled),
            ModelSource::Path(models.join("model.gguf"))
        );

        // バンドル外の実行ファイルは同梱ファイルを参照しない
        let dev_exe = dir.path().join("target/release/yuzukan-imserver");
        let relative = Path::new("models/owner/repo/model.gguf");
        assert_eq!(bundled_file_beside(&dev_exe, relative), None);
    }

    #[test]
    fn test_color_theme_setting_and_aliases() {
        assert_eq!(Settings::default().display.theme, ColorTheme::System);

        let cases = [
            ("yuzu", ColorTheme::Yuzu),
            ("yuzu-light", ColorTheme::YuzuLight),
            ("yuzu_light", ColorTheme::YuzuLight),
            ("yuzu-dark", ColorTheme::YuzuDark),
            ("yuzu_dark", ColorTheme::YuzuDark),
            ("yuzu-leaf", ColorTheme::YuzuLeaf),
            ("yuzu_leaf", ColorTheme::YuzuLeaf),
            ("yuzu-green", ColorTheme::YuzuLeaf),
            ("yuzu_green", ColorTheme::YuzuLeaf),
            ("system", ColorTheme::System),
            ("monochrome", ColorTheme::System),
        ];

        for (raw, expected) in cases {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, "[display]\ntheme = \"{raw}\"").unwrap();
            let settings = Settings::load_from(file.path()).unwrap();
            assert_eq!(settings.display.theme, expected, "raw theme = {raw}");
        }
    }
}
