workspace "Anonymous Roles" "Elements without identifiers" {

    model {
        person "Warehouse Operator" "Runs the floor"
        personDupQz = person "Warehouse Operator" "Runs the floor"

        softwareSystem "Scanner Terminal" "Barcode scanning client"

        person "Auditor" "Checks stock counts"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
