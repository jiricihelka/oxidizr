workspace "Broken Theme" "Installed theme whose theme.json is not valid JSON" {

    model {
        user = person "User"
        sys = softwareSystem "System"

        user -> sys "uses"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        theme broken
    }

}
