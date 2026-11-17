workspace "Modernisation Map" "Strangler pattern estate" {

    !identifiers hierarchical

    model {
        ops = person "Ops" "Runs both worlds"

        !include legacy.dsl
        !include newwave.dsl

        ops -> strangler "operates"
        strangler.kafka -> legacyCore.monolith "mirrors writes to"
    }

    views {
        systemLandscape "landscape" "Before and after" {
            include *
            autoLayout
        }

        container strangler "strangler-containers" "Inside the strangler" {
            include *
            autoLayout
        }
    }

}
