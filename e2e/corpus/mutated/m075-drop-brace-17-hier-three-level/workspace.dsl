workspace "Billing Deep Dive" "Component level view of the billing API" {

    model {
        billing = softwareSystem "Billing" "Issues invoices" {
            app = container "Billing API" "REST surface" "Java" {
                invoicing = component "Invoicing" "Builds invoices"
                dunning = component "Dunning" "Chases unpaid invoices"

                invoicing -> dunning "escalates overdue to"
            
            db = container "Billing Store" "Invoice archive" "PostgreSQL"

            app -> db "persists in"
        }
    }

    views {
        container billing "containers" {
            include *
            autoLayout
        }

        component app "components" {
            include *
            autoLayout
        }
    }

}
