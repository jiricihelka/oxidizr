!const GROUP_NAME "Payments"
!const SEPARATOR "/"

workspace "Consts Grouping" "Constants feeding group names" {

    properties {
        "structurizr.groupSeparator" "${SEPARATOR}"
    }

    model {
        group "${GROUP_NAME} Domain" {
            wallet = softwareSystem "Wallet" "Stored value"
            billing = softwareSystem "Billing" "Invoices"

            wallet -> billing "pays via"
        }
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    

}
