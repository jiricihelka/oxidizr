workspace "Nested Anonymous" "Containers and components without identifiers" {

    model {
        erp = softwareSystem "ERP" "Back office" {
            container "Finance Module" "Ledgers and AP/AR" "ABAP"
            container "HR Module" "Payroll and records" "ABAP"

            container "Integration Bus" "RFC bridge" {
                component "IDoc Adapter" "Parses IDocs"
            }
        }
    }

    views {
        container erp "containers" {
            include *
            autoLayout
        }
    }

}
