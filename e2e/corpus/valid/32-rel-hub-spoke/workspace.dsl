workspace "Hub And Spoke" "One hub, many spokes" {

    model {
        hub = softwareSystem "Integration Hub" "Enterprise service bus"
        sap = softwareSystem "SAP ECC" "Finance and logistics"
        shop = softwareSystem "Web Shop" "Direct sales"
        wms = softwareSystem "WMS" "Warehouse management"
        tms = softwareSystem "TMS" "Transport management"
        crm = softwareSystem "CRM" "Customer 360"

        shop -> hub "orders via"
        hub -> sap "posts to"
        hub -> wms "dispatches to"
        hub -> tms "plans shipments via"
        hub -> crm "syncs customers to"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
