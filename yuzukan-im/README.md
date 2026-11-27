# yuzukan-im

yuzukan の IME 本体。共有エンジンと各プラットフォームのフロントエンドがここにまとまっています。

| ディレクトリ | 説明 |
|---------|------|
| [core/](core/) | 共有IMEエンジン(crate: `yuzukan-im`) — ステートマシン、ローマ字変換、yuzukan-imserver(macOS向けJSON-RPCサーバー) |
| [macos/](macos/) | macOS向けフロントエンド — Swift/InputMethodKit |

キーバインド・設定・辞書などユーザー向けドキュメントは [docs/](../docs/) を、インストール手順は各フロントエンドの README を参照してください。
