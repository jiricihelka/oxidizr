workspace "Filtered Include" "Include-only filter" {

    model {
        staff = person "Staff"
        crm = softwareSystem "CRM" "Sales" Sales
        erp = softwareSystem "ERP" "Finance" Finance

        staff -> crm "works in" "" Sales
        staff -> erp "works in" "" Finance
    }

    views {
        systemLandscape "all" {
            include *
        }

        filtered all include Sales "sales-only" "Sales slice."
    }

}
