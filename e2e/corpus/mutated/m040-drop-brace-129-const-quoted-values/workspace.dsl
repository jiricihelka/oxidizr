!const TEAM "pay-team"
!const OWNER_EMAIL "pay@example.com"

workspace "Quoted Constants" "Quoted constant values" {

    model {
        wallet = softwareSystem "Wallet" "Owned by ${TEAM}" {
            properties {
                "owner" "${OWNER_EMAIL}"
                "team" "${TEAM}"
            }
        
    }

}
