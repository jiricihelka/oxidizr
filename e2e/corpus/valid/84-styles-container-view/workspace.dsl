workspace "Styles On Containers" "Technology-based styling" {

    !identifiers hierarchical

    model {
        data = softwareSystem "Data Platform" "Pipes" {
            stream = container "Stream" "Kafka" "Kafka" {
                tags "Streaming"
            }
            lake = container "Lake" "Objects" "S3" {
                tags "Storage"
            }

            stream -> lake "lands"
        }
    }

    views {
        container data "containers" {
            include *
            autoLayout
        }

        styles {
            element "Streaming" {
                shape RoundedBox
                background #364fc7
                color #ffffff
            }

            element "Storage" {
                shape Cylinder
                background #2b8a3e
            }
        }
    }

}
