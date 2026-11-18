workspace "Escaped Names" "CJK, quotes, brackets and backslashes survive the dialects" {

    !identifiers hierarchical

    model {
        user = person "用户 \"Uomo\" [ cliente ]" "Multi\nline description (quoted)"

        gateway = softwareSystem "网关 «Pass-through» (prod)" "Backslash \\ path like C:\\tmp" {
            tags "Gateway"
        }

        store = softwareSystem "Store" {
            cache = container "缓存 {Core}" "Braces in description"
            db = container "DB [Postgres]" "Parens (and) squares"
        }

        group "Zona «Alpha»" {
            aux = softwareSystem "Aux"
        }

        user -> gateway "pásalo → ok"
        gateway -> store.cache "reads «cache»"
        store.cache -> store.db "falls back"
        user -> aux "uses"
    }

    views {
        systemLandscape "landscape_v2-x" "Punctuation in the view key (dots are rejected by the key charset)" {
            include *
            autoLayout
        }

        styles {
            element "Gateway" {
                background "#E8F0FE"
                stroke "#1A56DB"
            }
        }
    }

}
