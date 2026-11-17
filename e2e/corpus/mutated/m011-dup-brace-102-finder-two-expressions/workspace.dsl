workspace "Finder Two Expressions" "Two finders over different sets" {

    model {
        order = softwareSystem "Order" "Orders" {
            front = container "Front" "UI" "TypeScript"
            back = container "Back" "Service" "Java"
        }

        !elements "element.technology==TypeScript" {
            tag "Frontend"
        }
        }

        !elements "element.technology==Java" {
            tag "Backend"
        }
    }

}
