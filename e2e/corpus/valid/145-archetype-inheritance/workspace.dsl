workspace "Archetype Inheritance" "Archetype extending archetype" {

    model {
        archetypes {
            service = container {
                technology "Java"
                tags "Service"
            }

            springService = service {
                technology "Spring Boot"
                tags "Spring"
            }
        }

        core = softwareSystem "Core" "Business core" {
            api = springService "Core API" "REST"
            legacy = service "Old Batch" "Jobs"
        }
    }

}
