workspace "Minimal Styles" "One selector one property" {

    model {
        s = softwareSystem "Something" {
            tags "Highlighted"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
        }

        styles {
            element "Highlighted" {
                background #ffd43b
            }
        }
    }

}
