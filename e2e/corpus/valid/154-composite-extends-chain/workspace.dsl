workspace extends sales.dsl {

    model {
        !element erp {
            finance = container "Finance Core" "Ledger" "ABAP"
        }

        core -> finance "syncs customers to"
    }

    views {
        container erp "erp-containers" "Inside ERP" {
            include *
            autoLayout
        }
    }

}
