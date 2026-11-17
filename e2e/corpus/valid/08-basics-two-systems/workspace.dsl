workspace "Transit Grid" "Two systems exchanging timetable data" {

    model {
        planner = softwareSystem "Trip Planner" "Plans journeys across operators"
        feed = softwareSystem "GTFS Feed" "Publishes GTFS datasets"

        feed -> planner "feeds schedules to"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
