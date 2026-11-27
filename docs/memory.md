# メモリと最適化

## 測定結果

Apple Silicon の Mac で、システム辞書（約 205 MB）とモデルを読み込み、ライブ変換・Space 変換・上段のカナ選択を実行した状態の値です。

| 対象 | `phys_footprint` | ピーク `phys_footprint` | Clean な `mapped file` | RSS（`ps`） |
|---|---:|---:|---:|---:|
| エンジン（`yuzukan-imserver`、`adaptive`） | 51 MB | 70 MB | 299 MB | 361.3 MiB |
| フロントエンド（`YuzukanIME`） | 22 MB | 23 MB | 14 MB | 90.0 MiB |
| 合計（`adaptive`、既定） | 73 MB | 93 MB | 313 MB | 451.3 MiB |
| 参考: エンジン（`main`） | 50 MB | 60 MB | 273 MB | 334.4 MiB |

- `phys_footprint` は、アクティビティモニタの「メモリ」にあたる値です。`footprint <プロセスID>` で確認できます。
- 合計のピークは、2 つのプロセスのピークを足した値です。
- `mapped file`（Clean）はファイルに対応するメモリマップ領域です。メモリが不足すると、macOS はディスクへ書き出さずに解放できます。RSS にはこの領域が含まれるため、`phys_footprint` より大きくなります。

## さらに抑える

`config.toml` で `strategy = "main"` にすると、メインモデルだけを読み込み、ピークとマップ領域が小さくなります。代わりに軽量モデルへの自動切り替えと、AI の別候補（ビームサーチ）が使えなくなります。詳しくは [設定の Conversion Strategy](configuration.md#conversion-strategy) を参照してください。

## 行った最適化

### CPU 推論とモデル

- `GGML_METAL=OFF` で、使わない GPU の初期化を省きます（読み込みが 5.2 秒程度から 0.4 秒程度になりました）。
- `GGML_CPU_REPACK=OFF` で、CPU 向けに並べ替えた重みの複製を作りません。
- 内容が同じ `tokenizer.json` は SHA-256 で判定して共有します。

### 辞書

- `dict.bin` をプライベート `mmap` で必要な分だけ読み込みます。
- APFS の `clonefile` を使い、`fsync` を省くことで、起動時とスリープ復帰時の辞書の複製をなくしました。
- 索引の位置を `u32` にして、索引のメモリを半分にしました。

### 変換表と入出力

- 絵文字・単漢字・記号の表（YAML で計約 1.56 MB）を `build.rs` で静的な配列にし、実行時の解析をなくしました。
- 学習キャッシュを `BTreeMap` で持ち、保存はアトミックに行います。
- 絵文字の入力中に出す候補は 30 件までです。
- ログは起動時に 512 KiB でローテーションします。

### フロントエンド

- 候補の行のビューを再利用します。
- JSON-RPC の応答を二重に解析しないようにしました。
- 単語ごとのデバッグログは、環境変数 `YUZUKAN_DEBUG_LOG=1` を指定したときだけ出します。
