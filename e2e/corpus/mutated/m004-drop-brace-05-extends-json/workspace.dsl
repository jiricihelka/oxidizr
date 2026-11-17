workspace extends base.json {

    model {
        portal = softwareSystem "Portal" "Customer-facing portal"

        Clerk -> portal "answers tickets in"
        portal -> Ledger "reads balances from"
    

}
