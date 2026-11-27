# yuzukan macOS

macOS向けyuzukan日本語入力(InputMethodKit + Swift)。

## アーキテクチャ

サーバー・クライアント構成です。IMEの状態機械・ローマ字変換・かな漢字変換はすべて
Rust側(`yuzukan-im`)にあり、Swift側はInputMethodKitとの橋渡しに徹します。

```
┌───────────────────────────────┐                            ┌─────────────────────────────┐
│ YuzukanIME (Swift)            │  JSON-RPC 2.0 (改行区切り)  │ yuzukan-imserver (Rust)     │
│ ・IMKServer / InputController │ ◀────────────────────────▶ │ ・InputMethodEngine         │
│ ・NSEvent → XKB keysym変換    │    stdin/stdout パイプ      │ ・ローマ字変換・状態機械    │
│ ・preedit / 候補ウィンドウ描画 │                            │ ・llama.cpp推論・辞書・学習 │
│ ・子プロセス管理・自動再起動   │                            │                             │
└───────────────────────────────┘                            └─────────────────────────────┘
        どちらも yuzukan.app バンドル内 (Contents/MacOS/)
```

- プロトコル定義: `yuzukan-im/core/src/server/protocol.rs`(Rust側が正)と
  `Sources/YuzukanIME/EngineProtocol.swift`(Swift側ミラー)
- キーイベントはfcitx5版と同じXKB keysym表現に変換して送る
  (`Sources/YuzukanIME/KeyCodeMap.swift`)
- エンジンプロセスはクラッシュ時に指数バックオフで自動再起動、
  スリープ復帰時にも再起動(macOSがスリープ中にパイプを破棄するため)

## インストール

### 初回インストール

#### 前提条件

- Xcodeコマンドラインツール(`swift`、`make`): `xcode-select --install`
- [Rust](https://www.rust-lang.org/tools/install)(`cargo`。1.99.1で動作確認)
- CMake

```bash
cd yuzukan-im/macos

# ビルド + .appバンドル組み立て + ~/Library/Input Methods へインストール
make install
```

その後、以下の手順で利用できます:

1. macOSからログアウトし、再ログイン(macOSが新しいIMEを認識するために必要)
2. 「システム設定」>「キーボード」>「入力ソース」を編集 >「+」ボタン>「日本語」> **yuzukan** を追加
3. メニューバーの入力メニューからyuzukanを選択

> **Note:** `make install` 実行時に変換モデル(設定の `[models]` に定義された全モデル)を
> Hugging Faceからダウンロードしてキャッシュ(`~/.cache/huggingface/`)に配置します。
> パス文字列で指定したローカルモデルはダウンロードせず、ファイルがあるかだけ確認します。
> オフラインなどでダウンロードに失敗した場合は、IMEの起動時にバックグラウンドで
> ダウンロードされます。その間もかな入力と辞書変換はそのまま使え、モデルの読み込みが
> 完了すると自動でモデル変換が有効になります。

### 開発版の更新(2回目以降)

ログアウトは不要です。インストール後にアプリを再起動します:

```bash
make install
```

IMEだけを再起動する場合は以下を実行してください。内部のプロセス名は`YuzukanIME`です。

```bash
killall YuzukanIME
open "$HOME/Library/Input Methods/yuzukan.app"
```

ただし**コード以外のメタデータ**(メニューアイコン、入力モード名、Info.plistの
入力モード定義)はmacOS側にキャッシュされるため、`killall YuzukanIME` では反映され
ないことがあります。その場合は次のいずれかで反映されます:

```bash
# 入力メニューのアイコン・名前のキャッシュを更新(エージェントは自動再起動)
killall TextInputMenuAgent
```

- それでも反映されない場合: システム設定 → 入力ソースからyuzukanを削除 → 再追加
- 最終手段: ログアウト → ログイン

### テスト

```bash
# Swift + Rustサーバー統合テスト
make test
```

## 配布用 pkg の作成

利用者が開発環境なしで導入できる pkg を作ります。利用者側の手順は
[ルートの README](../../README.md) の「pkg でインストールする」を参照してください。

```bash
make pkg   # out/yuzukan-<バージョン>-arm64.pkg ができる
```

- 辞書 (`dict.bin`) と既定の2モデルを `yuzukan.app/Contents/Resources/` に同梱します。
  取得したファイルは `out/pkg-cache/` に残り、次回は再利用します。
- 利用者のデータディレクトリに `dict.bin` がある場合は、そちらを同梱の辞書より優先します。
- 対象は Apple Silicon、macOS 13 以降です。インストール先は `/Library/Input Methods` です。
- `make install` で導入した版がある場合は、重複を避けるため `~/Library/Input Methods/yuzukan.app` を削除してから pkg を入れます。
- pkg には署名がありません(公証もしていません)。
- 既定のモデルを変えるときは、`pkg/build.sh` の `MODELS` と `pkg/MODELS-LICENSE.txt` も直します。
- バージョンは `Info.plist` の `CFBundleShortVersionString` です。

## システム辞書のインストール

モデル推論だけでは語彙が限られるため、システム辞書の併用を強く推奨します。
`make install` 実行時に、辞書が未インストールであればGitHubリリースのビルド済み辞書
(`dict.tgz`)を自動でダウンロードして配置します。既に
`~/Library/Application Support/com.yuzukan.yuzukan-im/dict.bin` がある場合は
何もしません(自前ビルドの辞書が上書きされることはありません)。

手動でインストールする場合:

```bash
curl -LO https://github.com/togatoga/karukan/releases/latest/download/dict.tgz
tar xzf dict.tgz
mkdir -p ~/Library/"Application Support"/com.yuzukan.yuzukan-im
cp dict.bin ~/Library/"Application Support"/com.yuzukan.yuzukan-im/
killall YuzukanIME  # 起動中の場合は再起動して反映
```

辞書を自分でビルドする場合は [yuzukan-cli の README](../../yuzukan-cli/README.md) を参照してください。

## キー操作

[docs/key-bindings.md](../../docs/key-bindings.md) を参照してください（共通キーバインドに加えて、JISかな・英数キーや右⌘単独タップなどのmacOS固有キーをまとめています）。

## 設定・データファイル

- 設定: `~/Library/Application Support/com.yuzukan.yuzukan-im/config.toml`
- システム辞書: `~/Library/Application Support/com.yuzukan.yuzukan-im/dict.bin`
- ユーザー辞書: `~/Library/Application Support/com.yuzukan.yuzukan-im/user_dicts/`
- 学習データ: `~/Library/Application Support/com.yuzukan.yuzukan-im/learning.tsv`

> **Note:** 上記パスにファイルが存在しない場合、旧Karukanの保存先（`~/Library/Application Support/com.karukan.karukan-im/`）にある既存ファイルがフォールバックとして自動的に読み込まれます（`make install` も同ディレクトリに `dict.bin` があれば再利用します）。

設定項目は [docs/configuration.md](../../docs/configuration.md) を、辞書は [docs/dictionary.md](../../docs/dictionary.md) を参照してください。

## デバッグ

- ログ: `~/Library/Logs/YuzukanIME/yuzukan-ime.log`(Swift側NSLogとRust側tracingの両方)
- サーバー単体デバッグ: JSON-RPCを直接流せます

  ```bash
  cargo run -p yuzukan-im --bin yuzukan-imserver
  {"jsonrpc":"2.0","id":1,"method":"process_key","params":{"keysym":107}}
  ```

- バンドルを組み立てずに開発中のサーバーを使う: `YUZUKAN_IMSERVER=/path/to/yuzukan-imserver`

## 既知の制約

- ローマ字入力のみ対応(かな入力レイアウトは未対応)
- 候補ウィンドウはマウス操作不可(数字キー・矢印キーで選択)

## 参考プロジェクト

- [mac-akaza](https://github.com/akaza-im/mac-akaza)
