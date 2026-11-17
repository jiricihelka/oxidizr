workspace "Relationship Archetypes" "Typed links via archetypes" {

    model {
        archetypes {
            https = -> {
                technology "HTTPS"
                tags "Sync"
            }
            async = -> {
                technology "Kafka"
                tags "Async"
            }
        }

        web = softwareSystem "Web" "UI"
        api = softwareSystem "API" "Service"
        bus = softwareSystem "Bus" "Events"

        web --https-> api "calls"
        api --async-> bus "publishes to"
    }


