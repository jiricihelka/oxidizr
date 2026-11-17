workspace "Component Web" "Three components wired together" {

    model {
        portal = softwareSystem "Citizen Portal" "Government self-service" {
            web = container "Web App" "Server-rendered UI" "Play" {
                forms = component "Forms" "Guided interviews"
                docs = component "Document Store" "Uploads and templates"
                notify = component "Notifier" "E-mails status letters"

                forms -> docs "attaches uploads to"
                forms -> notify "triggers letters via"
            }
        }
    }

    views {
        component web "components" {
            include *
            autoLayout
        }
    }

}
