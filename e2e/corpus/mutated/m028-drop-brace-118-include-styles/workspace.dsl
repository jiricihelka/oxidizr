workspace "Included Styles" "Style selectors from a file" {

    model {
        pay = softwareSystem "Payments" "Money" {
            tags "Important"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
        }

        styles {
            !include styles/palette.dsl
        
    }

}
