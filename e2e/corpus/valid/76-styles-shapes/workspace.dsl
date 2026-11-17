workspace "Element Shapes" "Various shapes" {

    model {
        s1 = softwareSystem "Boxed" {
            tags "Boxed"
        }
        s2 = softwareSystem "Circular" {
            tags "Circular"
        }
        s3 = softwareSystem "Cylinder" {
            tags "Cylinder"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        styles {
            element "Boxed" {
                shape Box
                background #1168bd
                color #ffffff
            }

            element "Circular" {
                shape Circle
                background #23A2D9
            }

            element "Cylinder" {
                shape Cylinder
                background #8a2be2
            }
        }
    }

}
