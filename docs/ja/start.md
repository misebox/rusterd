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

await init();  // デフォルトエクスポート。wasm の読み込み。最初に一度だけ
```

**どの関数も `string` を返し、失敗したときは `string` を throw します。**
包みを開ける結果オブジェクトはありません。throw されるのは `Error` ではなく
文字列そのものなので、`catch (e) { e.message }` ではなく `catch (message)` です。

| 関数 | 返るもの |
| --- | --- |
| `erdToSvg` | `<svg …>…</svg>` のマークアップ |
| `erdToDataUri` | `data:image/svg+xml,…`。`<img src={…}>` にそのまま渡せる |
| `sqlToErd` | ERD のソース。`.erd` ファイルの中身と同じもの |
| `sqlToSvg` | マークアップ。変換を挟むだけ |

```typescript
erdToSvg(source: string, view?, detail?, notation?, legend?, dense?): string
erdToDataUri(source: string, view?, detail?, notation?, legend?, dense?): string
sqlToErd(sql: string, dialect?): string
sqlToSvg(sql: string, dialect?, view?, detail?, notation?, legend?, dense?): string
```

第 1 引数より後はすべて省略できます。途中を飛ばして後ろを指定したいときは `null`
を渡してください。この型定義はパッケージに同梱されているので、エディタも同じことを
教えてくれます。

| 引数 | 型 | 値 | 省略時 |
| --- | --- | --- | --- |
| `view` | `string \| null` | ソースが定義する `view` の名前 | 図の全体 |
| `detail` | `string \| null` | `tables`, `pk`, `pk_fk`, `all` | `all` |
| `notation` | `string \| null` | `crowsfoot`, `text` | `crowsfoot` |
| `legend` | `boolean \| null` | | 凡例なし |
| `dense` | `boolean \| null` | | 通常の間隔 |
| `dialect` | `string \| null` | `auto`, `generic`, `postgres`, `mysql` | `auto` |

```javascript
const whole = erdToSvg(source);                                // 図の全体
const part = erdToSvg(source, 'checkout');                     // ビュー 1 つ
const keys = erdToSvg(source, null, 'pk_fk', 'text');          // キーのみ・文字表記
const tight = erdToSvg(source, null, null, null, true, true);  // 凡例つき・間隔を詰める

document.querySelector('img').src = erdToDataUri(source);

try {
  erdToSvg('entity {');
} catch (message) {
  console.error(message);  // "Unexpected token: LBrace, expected identifier"
}
```

`sqlToErd` は解釈できない文を読み飛ばすので、まったく読めないダンプは throw ではなく
`''` が返ります。図を描く前に確かめてください:

```javascript
const erd = sqlToErd(dump, 'postgres');
if (!erd.trim()) {
  throw new Error('テーブルが見つかりません。SQL か、方言の指定を確認してください。');
}
```

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
