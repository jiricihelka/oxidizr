workspace "Wired By Tech" "Relationships carrying technologies" {

    model {
        mobile = softwareSystem "Mobile App" "iOS and Android client"
        gateway = softwareSystem "API Gateway" "Edge for north-south traffic"
        kafka = softwareSystem "Event Bus" "Kafka cluster"

        mobile -> gateway "calls" "HTTPS/JSON"
        gateway -> kafka "publishes events to" "Kafka"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }


