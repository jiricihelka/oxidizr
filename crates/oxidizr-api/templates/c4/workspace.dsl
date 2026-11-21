workspace "Web Shop" "A starter C4 system landscape created by `oxidizr init --template c4`." {
    model {
        customer = person Customer "A customer of the web shop."

        webShop = softwareSystem "Web Shop" "Allows customers to browse and buy products." {
            webApp = container "Web Application" "Provides the shop to customers via their web browser." JavaScript
            database = container Database "Stores products, customers and orders." PostgreSQL

            webApp -> database "Reads from and writes to"
        }

        paymentProvider = softwareSystem "Payment Provider" "Third-party provider that processes card payments." {
            tag "External System"
        }

        emailSystem = softwareSystem "E-mail System" "Sends order confirmations to customers." {
            tag "External System"
        }

        customer -> webShop "Browses and buys products via"
        webShop -> paymentProvider "Makes payments through"
        webShop -> emailSystem "Sends order confirmations through"
    }
    views {
        systemLandscape landscape {
            title "System Landscape"
            description "The big picture: the web shop and the systems it depends on."
            include *
            autoLayout
        }
        systemContext webShop context {
            title "System Context"
            description "The web shop and the users and systems it interacts with."
            include *
            autoLayout
        }
        container webShop containers {
            title Containers
            description "The containers inside the web shop."
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
            element "External System" {
                background #999999
                color #ffffff
            }
            element Container {
                background #23a2d7
                color #ffffff
            }
        }
    }
}
