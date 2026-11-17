workspace "AutoLayout Only" "Views relying on plain autoLayout" {

    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        c = softwareSystem "C"

        a -> b
        b -> c
    }

    views {
        systemLandscape "l1" {
            include *
            autoLayout
        }

        systemContext b "b1" {
            include *
            autoLayout
        }

        systemContext b "b2" "B again" {
            include *
            autoLayout
        }
    }

}
