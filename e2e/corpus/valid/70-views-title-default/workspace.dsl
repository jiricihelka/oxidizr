workspace "Default View" "Marking a view default" {

    model {
        desk = softwareSystem "Desk" "Service desk"
        chat = softwareSystem "Chat" "Support chat"

        desk -> chat "handoff to"
    }

    views {
        systemLandscape "landscape" {
            title "Estate Overview"
            include *
            autoLayout
        }

        systemContext desk "desk-context" {
            include *
            autoLayout

            default
        }
    }

}
