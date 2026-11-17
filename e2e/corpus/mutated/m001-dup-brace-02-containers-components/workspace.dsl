workspace "Order Service" "Containers and components" {

    !identifiers hierarchical

    model {
        customer = person "Customer"

        orders = softwareSystem "Orders" "Takes orders" {
            webapp = container "Web Application" "Serves the UI" "Spring Boot" {
                controller = component "Order Controller" "Handles order API calls" "Spring MVC"
                ui = component "Order UI" "Renders the order forms" "Thymeleaf"
            }
            db = container "Database" "Stores orders" "PostgreSQL"

            webapp.controller -> webapp.ui "renders"
            webapp -> db "reads and writes" "JDBC"
        }

        customer -> orders.webapp "places orders via" "HTTPS"
    }

    views {
        container orders "containers" {
            include *
            autoLayout
        }
        }

        component orders.webapp "components" {
            include *
            autoLayout
        }
    }

}
