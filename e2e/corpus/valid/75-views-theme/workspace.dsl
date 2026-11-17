workspace "Theme Default" "Using the default theme" {

    model {
        u = person "User"
        s = softwareSystem "System"

        u -> s "uses"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        theme default
    }

}
