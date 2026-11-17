workspace extends base.dsl {

    model {
        !element crm {
            portal = container "Portal" "Client self-service" "React"
        

        !element erp {
            core = container "Core" "Ledger" "ABAP"

            portal -> core "sends orders to"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
