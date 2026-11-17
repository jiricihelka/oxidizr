workspace "Composite Includes" "Estate, styles and views from files" {

    model {
        !include estate/systems.dsl

        adjuster = person "Adjuster" "Handles claims" {
            tags "Critical"
        }

        adjuster -> intake "works in"
    }

    views {
        !include viewing/diagrams.dsl

        styles {
            !include estate/styles.dsl
        }
    }

}
