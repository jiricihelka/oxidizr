workspace "Metadata Statements" "description/url/tags inside element blocks" {

    model {
        billing = softwareSystem "Billing" {
            description "Issues invoices and receipts"
            url "https://billing.example.com"
            tags "Critical,Money"
            properties {
                "owner" "fin-team"
            }
        }

        ops = person "Ops Engineer" {
            description "Keeps the lights on"
            tags "Internal"
        }

        ops -> billing "operates"
    }

}
