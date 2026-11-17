workspace "Finder Hierarchical" "Finders with hierarchical identifiers" {

    !identifiers hierarchical

    model {
        portal = softwareSystem "Portal" "Public" {
            web = container "Web" "UI" "Go" {
                form = component "Form" "Input"
                send = component "Sender" "Mail"
            }
        }

        !elements "element.parent==portal.web" {
            tags "Flow"
        }

portal.web.form -> ghostRelRefQz "passes data to"
    }

}
