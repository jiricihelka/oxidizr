workspace "Clinic Systems" "Health estate across domains" {

    !identifiers hierarchical

    properties {
        "structurizr.groupSeparator" "/"
    }

    model {
        !include people.dsl
        !include domains

        patient -> booking "books via"
        clinician -> records "documents in"
        booking.slotsApi -> records.ehrApi "links appointments to"
    }

    views {
        systemLandscape "landscape" "Clinic landscape" {
            include *
            autoLayout
        }

        container booking "booking-containers" {
            include *
            autoLayout
        }

        container records "records-containers" {
            include *
            autoLayout
        }
    }

}
