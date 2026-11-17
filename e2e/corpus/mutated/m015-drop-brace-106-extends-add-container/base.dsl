workspace "Base Estate" "Systems live here" {

    model {
        crm = softwareSystem "CRM" "Sales"
        erp = softwareSystem "ERP" "Finance"

        crm -> erp "posts invoices to"
    }

}
