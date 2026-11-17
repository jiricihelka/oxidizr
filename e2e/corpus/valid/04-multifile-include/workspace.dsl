workspace "Multi File Estate" "Model split over included files" {

    model {
        !include model/people.dsl
        !include model/systems.dsl
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }

        systemContext support "support-context" {
            include *
            autoLayout
        }
    }

}
