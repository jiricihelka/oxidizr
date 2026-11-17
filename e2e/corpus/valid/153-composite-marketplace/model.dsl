buyer = person "Buyer" "Purchases goods"
seller = person "Seller" "Lists goods"

marketplace = softwareSystem "Marketplace" "Trading platform" {
    storefront = container "Storefront" "Catalog UI" "Next.js"
    orders = container "Orders" "Order lifecycle" "Kotlin"
    ledger = container "Ledger" "Payouts" "Kotlin"
    postgres = container "Postgres" "Primary store" "PostgreSQL"

    storefront -> orders "places orders"
    orders -> ledger "settles via"
    orders -> postgres "stores in"
    ledger -> postgres "books in"
}
stripe = softwareSystem "Stripe" "Payments"
shipco = softwareSystem "ShipCo" "Carrier"

buyer -> marketplace "buys via"
seller -> marketplace "lists via"
marketplace.storefront -> stripe "takes payments" "HTTPS"
marketplace.orders -> shipco "books pickups" "REST"
