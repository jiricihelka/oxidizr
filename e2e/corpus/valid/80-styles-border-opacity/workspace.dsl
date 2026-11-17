workspace "Borders And Opacity" "Dotted borders and opacity" {

    model {
        k = softwareSystem "Keep" {
            tags "Keep"
        }
        g = softwareSystem "Ghost" {
            tags "Ghost"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
        }

        styles {
            element "Keep" {
                border Solid
                opacity 100
                background #4dabf7
            }

            element "Ghost" {
                border Dotted
                opacity 30
                background #ced4da
            }
        }
    }

}
