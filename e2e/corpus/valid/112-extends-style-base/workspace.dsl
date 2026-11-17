workspace extends base.dsl {

    model {
        other = softwareSystem "Other" "Another" {
            tags "Accent"
        }

        thing -> other "relates to"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
