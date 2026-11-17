workspace {
    model {
        a = softwareSystem "A"
    }
    views {
        systemLandscape "l" {
            include *
        }
        plantuml l
    }
}
