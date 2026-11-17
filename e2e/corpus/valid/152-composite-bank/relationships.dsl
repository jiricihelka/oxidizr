retailCustomer -> onlineBanking "checks balances via"
onlineBanking.singlePageApplication -> onlineBanking.backend "calls" "HTTPS/JSON"
onlineBanking.backend -> mainframe.creditCardSystem "requests statements" "HTTPS"
onlineBanking.backend -> emailSystem "sends alerts via" "SMTP"
