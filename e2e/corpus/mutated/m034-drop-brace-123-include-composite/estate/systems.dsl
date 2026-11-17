intake = softwareSystem "Intake" "Catches claims"
assess = softwareSystem "Assessment" "Prices them"
pay = softwareSystem "Settlements" "Pays out"

intake -> assess "routes to"
assess -> pay "approves into"
