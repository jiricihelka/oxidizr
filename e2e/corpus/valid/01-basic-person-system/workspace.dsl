workspace "Getting Started" "The canonical minimal workspace" {

    !identifiers hierarchical

    model {
        user = person "User" "A user of the system"

        system = softwareSystem "System" "The system" {
            user -> this "Uses"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
