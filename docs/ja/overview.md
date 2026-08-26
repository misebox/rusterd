## できること

- **エンティティ** — 型付きの列を持つテーブルを書く
- **列の型** — `int`, `string`, `decimal`, `timestamp`, `boolean`, `text`
- **制約** — `pk`, `fk -> Entity.column`, `not null`, `unique`
- **関係** — 多重度は `1`, `*`, `0..1`, `1..*` の 4 種類
- **自己参照** — 自分自身を参照するエンティティ
- **自動配置** — レベル・レベル内の順序・横位置をすべて計算する。ヒントは「どこに置くか」ではなく「何が大事か」を書く
- **フォーカス** — `focus` ブロックでスキーマの一部だけを描く
- **詳細度** — 表名のみ / 主キー / 主キーと外部キー / 全列

## 例

`examples/sample.erd` と、`rusterd render` が描くもの:

```erd
# Self-referential entity
entity Category {
    id int pk
    parent_id int fk -> Category.id
    name string not null
}

entity User {
    id int pk
    email string unique not null
    name string
    created_at timestamp
}

entity Product {
    id int pk
    category_id int fk -> Category.id
    name string not null
    price decimal
    is_active boolean
}

entity Order {
    id int pk
    user_id int fk -> User.id
    total decimal
    status string not null
}

# All cardinality types: 1, *, 0..1, 1..*
rel {
    Category 1 -- * Category : "parent"
    Category 1 -- * Product
    User 1 -- * Order : "places"
    User 0..1 -- 1..* Product : "favorites"
}

# 図の一部に名前を付ける
focus simple {
    include User, Order
}
```
