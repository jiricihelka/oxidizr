workspace "Style Kitchen" "Many style properties at once" {

    model {
        p = person "Person"
        sys = softwareSystem "System" {
            tags "Hero"
        }

        p -> sys "uses" "Trusted"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        styles {
            element "Element" {
                strokeWidth 1
                fontSize 14
                opacity 90
            }

            element "Hero" {
                shape Hexagon
                background #0c66e4
                stroke #0a4f9c
                border Dashed
                width 400
                height 200
                metadata true
                description false
            }

            relationship "Relationship" {
                routing Orthogonal
                style Solid
                width 2
                position 40
            }

            relationship "Trusted" {
                color #2f9e44
                routing Curved
            }
        }
    }

}
