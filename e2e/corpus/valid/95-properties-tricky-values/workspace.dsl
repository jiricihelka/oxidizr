workspace "Tricky Property Values" "Values with commas, equals and quotes" {

    model {
        gate = softwareSystem "API Gateway" "Edge" {
            properties {
                "cors.origins" "https://a.example.com,https://b.example.com"
                "limits" "rps=100; burst=200"
                "note" "see ticket OPS-123"
            }
        }
    }

}
