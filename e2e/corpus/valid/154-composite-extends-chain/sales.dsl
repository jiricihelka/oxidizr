workspace extends org.dsl {

    model {
        !element crm {
            portal = container "Sales Portal" "Reps UI" "React"
            core = container "CRM Core" "Pipeline" "Java"

            portal -> core "calls"
        }
    }

    views {
        container crm "crm-containers" "Inside CRM" {
            include *
            autoLayout
        }
    }

}
