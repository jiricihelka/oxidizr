onlineBanking = softwareSystem "${BANK} Online Banking" "Online and mobile banking" {
    singlePageApplication = container "Single Page Application" "Browser UI" "Angular"
    mobileApplication = container "Mobile Application" "iOS/Android" "Swift/Kotlin"
    backend = container "Backend" "APIs" "Spring Boot" {
        authComponent = component "Auth" "Login"
        accountsComponent = component "Accounts" "Balances"
        authComponent -> accountsComponent "loads"
    }
    database = container "Database" "Accounts" "Oracle"

    backend -> database "reads and writes"
}
mainframe = softwareSystem "Mainframe Core Banking" "The legacy core" {
    creditCardSystem = container "Credit Card System" "Cards" "COBOL"
}
emailSystem = softwareSystem "E-mail System" "Outbound mail"
