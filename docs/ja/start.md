## インストール

| 使い方 | コマンド |
| --- | --- |
| ブラウザ / バンドラ | `npm i rusterd`（`bun add` / `pnpm add` も可） |
| コマンドライン | `cargo install rusterd` |
| Rust ライブラリ | `rusterd = { git = "https://github.com/misebox/rusterd" }` |

## コマンドライン

```bash
# ファイルに書き出す
rusterd render input.erd -o output.svg

# focus ブロックが挙げたものだけを描く
rusterd render input.erd -f simple -o output.svg

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

# バージョンを表示する
rusterd --version
```

## オプション

コマンドラインではフラグ、ブラウザではオプションオブジェクトのフィールド。
名前も値も同じ 6 つです。どれも省略でき、省略することが既定値の指定です。

| オプション | コマンドライン | フィールド | 値 | 既定 |
| --- | --- | --- | --- | --- |
| フォーカス | `-f`, `--focus <name>` | `focus` | `focus` ブロックの名前 | 図の全体 |
| 詳細度 | `-d`, `--detail <level>` | `detail` | `tables`, `pk`, `pk_fk`, `all` | `all` |
| 記法 | `-n`, `--notation <name>` | `notation` | `crowsfoot`, `text` | `crowsfoot` |
| 凡例 | `-l`, `--legend` | `legend` | — | なし |
| 密 | `-D`, `--dense` | `dense` | — | なし |
| 方言 | `-d`, `--dialect <name>` | `dialect` | `auto`, `generic`, `postgres`, `mysql` | `auto` |

**詳細度** はエンティティのどこまでを描くか。`tables` は名前だけ、`pk` は主キー、
`pk_fk` は主キーと外部キー、`all` は全列です。

**記法** は多重度の描き方。`crowsfoot` は線の上にカラスの足で、`text` は
`1` / `0..1` / `*` / `1..*` を線の脇に文字で置きます。**凡例** はその 4 つの
読み方を図の下に付けます。使っている記法で描かれます。

**密** はエンティティ間と線まわりの余白を詰めます。文字は読める大きさのままなので、
縮小表示とは別物です。逆に広げる設定はありません。

**方言** だけは描画ではなく読み取りの設定です。コマンドラインでは、図ではなく ERD を
書き出す `rusterd convert` のオプションで、描画のオプションは `rusterd render` に
付きます。どちらも `-d` なのはそのためです。ブラウザの `sqlToSvg` は読み取りと描画を
1 回で行うので、6 つすべてを受け取ります。

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
type Detail = "tables" | "pk" | "pk_fk" | "all";
type Notation = "crowsfoot" | "text";
type Dialect = "auto" | "generic" | "postgres" | "mysql";

erdToSvg(source: string, options?: DrawOptions | null): string
erdToDataUri(source: string, options?: DrawOptions | null): string
sqlToErd(sql: string, dialect?: Dialect | null): string
sqlToSvg(sql: string, options?: ConvertOptions | null): string

interface DrawOptions {
  focus?: string | null;
  detail?: Detail | null;
  notation?: Notation | null;
  legend?: boolean | null;
  dense?: boolean | null;
}

interface ConvertOptions extends DrawOptions {
  dialect?: Dialect | null;
}
```

この型定義はパッケージに同梱されています。値は型なので、エディタが補完し、
綴り間違いは TypeScript が弾きます。JavaScript には実行時に伝えます。既定値で
黙って描くのではなく、知らない値は throw します。

```javascript
erdToSvg(source, { detail: 'pkfk' });
// Unknown detail: "pkfk" (expected "tables", "pk", "pk_fk", "all")
```

必要なものだけ書けば済みます。

```javascript
const whole = erdToSvg(source);
const part = erdToSvg(source, { focus: 'checkout' });
const keys = erdToSvg(source, { detail: 'pk_fk', notation: 'text' });
const tight = erdToSvg(source, { legend: true, dense: true });

document.querySelector('img').src = erdToDataUri(source);

const svg = sqlToSvg(dump, { dialect: 'postgres', detail: 'pk_fk' });

try {
  erdToSvg('entity {');
} catch (message) {
  console.error(message);  // "line 1, column 8: expected a name, found `{`"
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
