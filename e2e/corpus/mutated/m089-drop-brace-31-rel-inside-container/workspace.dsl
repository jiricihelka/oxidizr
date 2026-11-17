workspace "Inside The Box" "Relationships declared inside contexts" {

    model {
        crm = softwareSystem "CRM" "Sales cloud" {
            frontend = container "CRM Frontend" "Sales UI" "React" {
                pipeline = component "Pipeline Board" "Deal stages"
                contacts = component "Contacts" "People and accounts"

                pipeline -> contacts "opens contacts of"
            }
            backend = container "CRM Backend" "Business logic" "Java"

            frontend -> backend "queries and mutates"
        }
    }

    views {
        container crm "containers" {
            include *
            autoLayout
        }

        component frontend "components" {
            include *
            autoLayout
        }
    }


