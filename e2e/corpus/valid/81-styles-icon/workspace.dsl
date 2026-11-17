workspace "Icons" "Icon placement on elements" {

    model {
        db = softwareSystem "Database" "Store" {
            tags "Database"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
        }

        styles {
            element "Database" {
                shape Cylinder
                icon "https://icons.example.com/db.svg"
                iconPosition Top
            }
        }
    }

}
