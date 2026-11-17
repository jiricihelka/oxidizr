workspace "Jumps And Routing" "Jump styles on links" {

    model {
        a = softwareSystem "A"
        b = softwareSystem "B"
        c = softwareSystem "C" {
            tags "Skip"
        }

        a -> b "to B"
        a -> c "crossing" "Skip"
        b -> c "to C"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        styles {
            relationship "Relationship" {
                routing Curved
            }

            relationship "Skip" {
                jump true
                color #e8590c
            }
        }
    }

}
