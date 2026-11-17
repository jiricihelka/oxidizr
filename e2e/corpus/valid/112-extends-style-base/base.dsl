workspace "Style Base" "Styles defined in base" {

    model {
        thing = softwareSystem "Thing" "A thing" {
            tags "Accent"
        }
    }

    views {
        styles {
            element "Accent" {
                background #1971c2
                color #ffffff
            }
        }
    }

}
