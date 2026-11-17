booking = softwareSystem "Booking" "Slots" {
    slotsApi = container "Slots API" "Availability" "Go"
    slotsDb = container "Slots DB" "Slot rows" "Postgres"

    slotsApi -> slotsDb "reads"
}
