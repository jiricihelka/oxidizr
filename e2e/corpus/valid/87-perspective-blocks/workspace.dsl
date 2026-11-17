workspace "Perspective Blocks" "Full perspective context with url" {

    model {
        ledger = softwareSystem "Ledger" "Books entries" {
            perspectives {
                perspective "Security" {
                    value "High"
                    description "Money moves here"
                    url "https://wiki.example.com/security/ledger"
                }
                perspective "Availability" {
                    value "Medium"
                }
            }
        }
    }

}
