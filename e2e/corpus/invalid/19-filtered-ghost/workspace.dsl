workspace {
    model {
        a = softwareSystem "A"
    }
    views {
        filtered ghost "x" {
            include *
        }
    }
}
