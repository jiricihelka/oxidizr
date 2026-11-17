workspace "Landscape Full" "Landscape with title, description, properties" {

    model {
        citizen = person "Citizen" "End user"
        permits = softwareSystem "Permits" "Applies and issues"
        payments = softwareSystem "Payments" "Takes fees"

citizen -> ghostRelRefQz "applies via"
        permits -> payments "charges via"
    }

    views {
        systemLandscape "landscape" "The whole estate" {
            title "Landscape"
            description "All systems"

            include *

            properties {
                "oxidizr.view" "full-landscape"
            }

            autoLayout

            default
        }
    }

}
