workspace "Relationship Styles" "Dashed thick coloured links" {

    model {
        a = softwareSystem "A"
        b = softwareSystem "B" {
            tags "Async"
        }

        a -> b "calls" "Async"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        styles {
            relationship "Async" {
                dashed true
                thickness 3
                color #ff0000
                fontSize 13
            }
        }
    }

}
