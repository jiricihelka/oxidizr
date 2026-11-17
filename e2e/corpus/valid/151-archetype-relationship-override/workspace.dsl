workspace "Archetype Link Override" "Archetype link with description and technology overrides" {

    model {
        archetypes {
            feed = -> {
                technology "gRPC"
                tags "Streaming"
            }
        }

        src = softwareSystem "Source" "Producer"
        sink = softwareSystem "Sink" "Consumer"

        src --feed-> sink "streams events" "gRPC stream" {
            tags "Streamed", "Critical"
        }
    }

}
