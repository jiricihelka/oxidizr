workspace "Grouped Containers In System" "Containers in a group carrying their own metadata" {

    model {
        crm = softwareSystem "CRM" "Sales" {
            group "Frontend" {
                spa = container "SPA" "React app" "React" {
                    tags "Frontend"
                    url "https://spa.example.com"
                    properties {
                        "layer" "frontend"
                    }
                }
                admin = container "Admin UI" "Back office" "React" {
                    tags "Frontend"
                }
            }

            group "Backend" {
                core = container "Core" "Business logic" "Java" {
                    tags "Backend"
                }

                spa -> core "calls"
            }
        }
    }

}
