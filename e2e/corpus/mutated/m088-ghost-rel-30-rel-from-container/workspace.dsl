workspace "Container Outbound" "A container reaching an external system" {

    model {
        payments = softwareSystem "Payments" "Card acquiring" {
            acquirer = container "Acquirer Adapter" "Speaks ISO 8583" "Java"
        }

        psp = softwareSystem "Card Network" "Scheme rails"

acquirer -> ghostRelRefQz "routes authorizations to" "ISO 8583"
    }

    views {
        container payments "containers" {
            include *
            autoLayout
        }
    }

}
