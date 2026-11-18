workspace "AutoLayout Variants" "Automatic layout header permutations per dialect" {

    model {
        user = person "User"
        front = softwareSystem "Frontend"
        back = softwareSystem "Backend"
        db = softwareSystem "Database"

        user -> front "uses"
        front -> back "calls"
        back -> db "reads"
    }

    views {
        systemLandscape "lr" "Left to right" {
            include *
            autoLayout lr
        }

        systemLandscape "tb-fine" "Top bottom, tuned separations" {
            include *
            autoLayout tb 800 2000 300 true
        }

        systemLandscape "bt-novert" "Bottom top, no vertices" {
            include *
            autoLayout bt 500 1500 250 false
        }

        systemLandscape "rl-min" "Right to left, defaults" {
            include *
            autoLayout rl
        }

        systemContext front "ctx" {
            include *
            autoLayout
        }
    }

}
