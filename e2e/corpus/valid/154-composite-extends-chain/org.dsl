!const ORG "Northwind"

workspace "${ORG} Estate" "Org-wide model" {

    model {
        staff = person "Staff" "Employee" {
            tags "Internal"
        }

        crm = softwareSystem "CRM" "Sales"
        erp = softwareSystem "ERP" "Finance"

        staff -> crm "sells in"
        crm -> erp "posts to"
    }

    views {
        systemLandscape "landscape" "Whole estate" {
            include *
            autoLayout
        }
    }

}
