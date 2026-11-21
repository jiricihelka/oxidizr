workspace "My Workspace" "A starter workspace created by `oxidizr init`." {
    model {
        user = person User "A user of my software system."
        system = softwareSystem "My Software System" "The software system I want to describe."

        user -> system Uses
    }
    views {
        systemLandscape landscape {
            title "System Landscape"
            description "The big picture of my software system."
            include *
            autoLayout
        }
        styles {
            element Person {
                shape person
                background #08427b
                color #ffffff
            }
            element "Software System" {
                background #1168bd
                color #ffffff
            }
        }
    }
}
