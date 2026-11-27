#!/bin/bash
# 辞書とモデルを同梱した配布用 pkg を作る(Apple Silicon 向け、署名なし)。
# `make pkg` から呼ぶ。配布用の設定で `make bundle` を実行し、
# 組み立てた out/yuzukan.app を元にする。
set -euo pipefail
cd "$(dirname "$0")/.." # yuzukan-im/macos

: "${DICT_URL:?DICT_URL を指定してください(make pkg を使う)}"

root_dir=$(cd ../.. && pwd)
APP=out/yuzukan.app
CACHE=out/pkg-cache
WORK=out/pkg
ROOT=$WORK/root
RES=$ROOT/yuzukan.app/Contents/Resources
IDENTIFIER=dev.otofu1024.inputmethod.yuzukan.pkg
MIN_OS=13.0
# core/config/default.toml の model と light_model に合わせる
MODELS=(
    "togatogah/jinen-v2-small.gguf jinen-v2-small-Q5_K_M.gguf"
    "togatogah/jinen-v2-xsmall.gguf jinen-v2-xsmall-Q5_K_M.gguf"
)

fail() {
    echo "ERROR: $1" >&2
    exit 1
}

# ビルドしたMacのパス(ユーザー名を含む)を配布物に残さないように置き換える。
# 開発用のビルドキャッシュとは設定が異なるため、出力先を分ける。
export CARGO_TARGET_DIR=$root_dir/target/pkg
export RUSTFLAGS="--remap-path-prefix=$HOME=/build --remap-path-prefix=$root_dir=/yuzukan"
export CFLAGS="-ffile-prefix-map=$HOME=/build -ffile-prefix-map=$root_dir=/yuzukan"
export CXXFLAGS=$CFLAGS
make bundle CARGO_TARGET="$CARGO_TARGET_DIR"

# URL をキャッシュに取得する(取得済みなら何もしない)
fetch() {
    [ -f "$2" ] && return 0
    mkdir -p "$(dirname "$2")"
    echo "Downloading $1"
    curl -fL --progress-bar -o "$2.part" "$1"
    mv "$2.part" "$2"
}

# 配布先で動かないバイナリを作らないための確認
for binary in "$APP"/Contents/MacOS/*; do
    [ "$(lipo -archs "$binary")" = "arm64" ] || fail "$binary が arm64 専用ではありません"
done
newer=$(
    find "$CARGO_TARGET_DIR"/release/build/llama-cpp-sys-2-*/out -name '*.a' -exec otool -l {} + |
        awk -v max="$MIN_OS" '$1 == "minos" && $2 + 0 > max + 0 { print $2 }' | sort -u
)
[ -z "$newer" ] || fail "llama.cpp が macOS $newer 向けにビルドされています。$CARGO_TARGET_DIR を削除してやり直してください"

fetch "$DICT_URL" "$CACHE/dict.tgz"
for model in "${MODELS[@]}"; do
    read -r repo file <<<"$model"
    for name in "$file" tokenizer.json; do
        fetch "https://huggingface.co/$repo/resolve/main/$name" "$CACHE/models/$repo/$name"
    done
done

rm -rf "$WORK"
mkdir -p "$ROOT" "$WORK/dict"
cp -R "$APP" "$ROOT/"

# 辞書とモデル
tar xzf "$CACHE/dict.tgz" -C "$WORK/dict"
cp "$WORK/dict/dict.bin" "$RES/"
for model in "${MODELS[@]}"; do
    read -r repo file <<<"$model"
    mkdir -p "$RES/models/$repo"
    cp "$CACHE/models/$repo/$file" "$CACHE/models/$repo/tokenizer.json" "$RES/models/$repo/"
done

# 同梱物のライセンス
mkdir -p "$RES/licenses/yuzukan" "$RES/licenses/dictionary" "$RES/licenses/models"
cargo metadata --format-version 1 --filter-platform aarch64-apple-darwin \
    --manifest-path ../../Cargo.toml |
    python3 pkg/collect_licenses.py "$RES/licenses"
cp ../../LICENSE-MIT ../../LICENSE-APACHE ../../THIRD_PARTY_LICENSES "$RES/licenses/yuzukan/"
cp "$WORK"/dict/docs/* "$RES/licenses/dictionary/"
cp pkg/MODELS-LICENSE.txt "$RES/licenses/models/"

# オブジェクトファイルのパスを含むデバッグ用シンボルを取り除く
for binary in "$ROOT"/yuzukan.app/Contents/MacOS/*; do
    strip -S "$binary"
    if LC_ALL=C grep -qaF -e "$HOME/" -e "$root_dir/" "$binary"; then
        fail "$binary にローカルのパスが残っています"
    fi
done

xattr -cr "$ROOT"
codesign --force --deep --sign - "$ROOT/yuzukan.app"
codesign --verify --deep --strict "$ROOT/yuzukan.app"

version=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" Info.plist)
output=out/yuzukan-$version-arm64.pkg

# インストーラが別の場所にある同じアプリを上書き先にしないようにする
pkgbuild --analyze --root "$ROOT" "$WORK/component.plist"
for key in BundleIsRelocatable BundleIsVersionChecked; do
    plutil -replace "0.$key" -bool false "$WORK/component.plist"
done
pkgbuild --root "$ROOT" \
    --component-plist "$WORK/component.plist" \
    --identifier "$IDENTIFIER" \
    --version "$version" \
    --install-location "/Library/Input Methods" \
    --scripts pkg/scripts \
    "$WORK/yuzukan-component.pkg"
sed "s/@VERSION@/$version/g" pkg/distribution.xml >"$WORK/distribution.xml"
productbuild --distribution "$WORK/distribution.xml" \
    --resources pkg/resources \
    --package-path "$WORK" \
    "$output"

echo "Created $(pwd)/$output"
