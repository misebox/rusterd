## インストール

| 使い方 | コマンド |
| --- | --- |
| ブラウザ / バンドラ | `npm i rusterd`（`bun add` / `pnpm add` も可） |
| コマンドライン | `cargo install --path .` — crates.io には未公開 |
| Rust ライブラリ | `rusterd = { git = "https://github.com/misebox/rusterd" }` |

## コマンドライン

```bash
# ファイルに書き出す
rusterd render input.erd -o output.svg

# ビューだけを描く
rusterd render input.erd -v simple -o output.svg

# 詳細度を指定する
rusterd render input.erd -d pk_fk -o output.svg

# 多重度の記法
rusterd render input.erd -n text -o output.svg

# 記号の凡例を付ける
rusterd render input.erd --legend -o output.svg

# 間隔を詰めて、大きなスキーマを 1 画面に収める
rusterd render input.erd --dense -o output.svg

# 標準入力から読む
cat input.erd | rusterd render - -o output.svg

# SQL ダンプを ERD 記法に変換する
rusterd convert schema.sql -o schema.erd
rusterd convert schema.sql -d postgres
```

**SQL の方言:** `auto`（既定）, `generic`, `postgres`, `mysql`

**詳細度:**
- `tables` — エンティティ名のみ
- `pk` — 主キー
- `pk_fk` — 主キーと外部キー
- `all` — 全列（既定）

**多重度の記法:**
- `crowsfoot` — 線の上にカラスの足で描く（既定）
- `text` — `1`, `0..1`, `*`, `1..*` を線の脇に文字で置く

`-D` / `--dense` はエンティティ間と線まわりの余白を詰めます。文字は読める大きさのままなので、縮小表示とは別物です。逆に広げる設定はありません。

## ブラウザ（WASM）

```javascript
import init, { erdToSvg, erdToDataUri, sqlToErd, sqlToSvg } from 'rusterd';

await init();

erdToSvg(source);                          // 図全体の SVG
erdToSvg(source, 'simple');                // 名前付きビュー
erdToSvg(source, null, 'pk_fk');           // 詳細度
erdToSvg(source, null, null, 'text');      // 多重度を文字で
erdToSvg(source, null, null, null, true);  // 凡例つき
erdToSvg(source, null, null, null, null, true);  // 間隔を詰める
erdToDataUri(source);              // data: URI（<img src={...}> にそのまま）
sqlToErd(sqlDump, 'postgres');     // SQL ダンプ → ERD 記法
sqlToSvg(sqlDump, 'postgres');     // SQL ダンプ → SVG
```

ソース以降の引数はすべて省略でき、`null` を渡せます。パースエラーや未知のビュー名は文字列として throw されます。

## Rust ライブラリ

パースし、グラフを組み、配置し、描画する:

```rust
use rusterd::ir::{DetailLevel, GraphIR};
use rusterd::layout::LayoutEngine;
use rusterd::parser::Parser;
use rusterd::svg::SvgRenderer;

let schema = Parser::new(source)?.parse()?;
let ir = GraphIR::from_schema(&schema, None, DetailLevel::All);
let layout = LayoutEngine::default().layout(&ir);
let svg = SvgRenderer::default().render(&ir, &layout);
```

SQL から ERD への変換は `rusterd::sql::parse_sql` と `rusterd::serializer::serialize` です。
