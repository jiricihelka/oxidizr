workspace "Filtered Views" "Include and exclude by tags" {

    model {
        user = person "User"
        now = softwareSystem "System Now" "Current" Current
        later = softwareSystem "System Later" "Future" Future

        user -> now "uses" "" Current
        user -> later "will use" "" Future
    }

    views {
        systemLandscape "all" "Everything" {
            include *
        }

        filtered all exclude Future "current-only" "What runs today."

        filtered all exclude Current "future-only" "The plan."
    }

}
