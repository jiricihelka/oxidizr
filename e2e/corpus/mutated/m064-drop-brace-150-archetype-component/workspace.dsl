workspace "Component Archetypes" "Component archetypes" {

    model {
        archetypes {
            controller = component {
                technology "Spring MVC"
                tags "Controller"
            }
            repository = component {
                technology "Spring Data"
                tags "Repository"
            }
        

        app = softwareSystem "App" "Monolith" {
            api = container "API" "REST" "Java" {
                usersCtl = controller "UsersController" "User endpoints"
                usersRepo = repository "UsersRepository" "User rows"

                usersCtl -> usersRepo "persists via"
            }
        }
    }

}
