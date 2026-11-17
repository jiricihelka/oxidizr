workspace "Selector Cascade" "Later selectors win" {

    model {
        crit = softwareSystem "Critical Thing" "Matters" "Backend,Critical"
        plain = softwareSystem "Plain Thing" "Does not matter" "Backend"
    }

    views {
        systemLandscape "landscape" {
            include *
        }

        styles {
            element "Backend" {
                background #495057
                color #ffffff
            }

            element "Critical" {
                background #c92a2a
                strokeWidth 4
            }
        }
    }

}
