workspace "Boxes With Metadata" "Custom landscape boxes with metadata" {

    model {
        dom = element "Insurance Domain" "Domain" "Motor insurance value stream" "Domain"
        crm = softwareSystem "Policy CRM" "Policies and customers"
        billing = softwareSystem "Policy Billing" "Direct debits"

crm -> ghostRelRefQz "sends premium orders to"
    }

    views {
        custom "domain-map" "Domain Map" "Where the systems live" {
            include *
            autolayout
        }
    }

}
