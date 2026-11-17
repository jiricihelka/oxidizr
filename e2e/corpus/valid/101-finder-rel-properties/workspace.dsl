workspace "Finder Relationship Payload" "Finders adding url and properties" {

    model {
        x = softwareSystem "X"
        y = softwareSystem "Y"

        x -> y "calls"

        !relationships "*->*" {
            tag "Traced"
            url "https://tracing.example.com"
            properties {
                "trace" "w3c"
            }
        }
    }

}
