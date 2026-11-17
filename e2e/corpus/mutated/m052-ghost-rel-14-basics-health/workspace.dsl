workspace "Clinic Link" "Outpatient scheduling" {

    model {
        nurse = person "Practice Nurse" "Books and triages"

        scheduling = softwareSystem "Scheduling System" "Manages appointment slots"
        ehr = softwareSystem "EHR" "Records clinical history"

nurse -> ghostRelRefQz "books slots in"
        scheduling -> ehr "reads patient records from"
    }

    views {
        systemContext scheduling "scheduling-context" {
            include *
            autoLayout
        }

        systemContext ehr "ehr-context" {
            include *
            autoLayout
        }
    }

}
