deploymentNode "web-1" "Web host" "EC2" "Linux" {
    webI = containerInstance web
}

deploymentNode "db-1" "DB host" "RDS" "Postgres" {
    dbI = containerInstance db
}
