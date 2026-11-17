workspace "Finder Inside System" "Component finders declared in the system" {

    !identifiers hierarchical

    model {
        user = person "User"

        sys = softwareSystem "System" "Has components" {
            app = container "Application" "Main" "Java" {
                one = component "One" "First"
                two = component "Two" "Second"

                !elements "element.parent==app" {
                    tags "Layered"
                    user -> this "uses"
                }
            }
        }
    }

}
