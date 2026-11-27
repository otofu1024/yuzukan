"""依存ライブラリのライセンス文を集める。

標準入力の `cargo metadata` の出力を読み、引数のディレクトリに
llama.cpp と各 Rust クレートのライセンスファイルをコピーする。
"""

import json
import pathlib
import shutil
import sys

output = pathlib.Path(sys.argv[1])
metadata = json.load(sys.stdin)
members = set(metadata["workspace_members"])
resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
summary = []

for package in metadata["packages"]:
    if package["id"] in members or package["id"] not in resolved:
        continue
    source = pathlib.Path(package["manifest_path"]).parent
    name = f"{package['name']}-{package['version']}"
    summary.append(f"{name}: {package['license'] or 'ライセンスファイルを参照'}")
    for file in sorted(source.iterdir()):
        if file.is_file() and file.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE")):
            destination = output / "rust" / name
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copy(file, destination / file.name)
    if package["name"] == "llama-cpp-sys-2":
        (output / "llama.cpp").mkdir(parents=True, exist_ok=True)
        shutil.copy(source / "llama.cpp" / "LICENSE", output / "llama.cpp" / "LICENSE")

if not (output / "llama.cpp" / "LICENSE").is_file():
    sys.exit("ERROR: llama.cpp のライセンスが見つかりません")
(output / "rust").mkdir(parents=True, exist_ok=True)
(output / "rust" / "CRATES.txt").write_text("\n".join(sorted(summary)) + "\n", encoding="utf-8")
