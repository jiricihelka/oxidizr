web = container "Web" "UI" "Go"
db = container "DB" "Store" "Postgres"

web -> db "reads"
