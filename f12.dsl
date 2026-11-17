workspace {
    model {
        e = softwareSystem "A"
        f = softwareSystem "B"
        r = e -> f "one"
        r2 = e -> f "two"
    }
}
