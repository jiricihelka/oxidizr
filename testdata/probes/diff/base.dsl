workspace "Diff probe" "P4-M0 fixture pair — base side" {
    model {
        staff = group "Staff" {
            alice = person "Alice" "Primary user"
        }
        bob = person "Bob"
        bank = softwareSystem "Bank" "Core banking" {
            web = container "Web" "Front end" "nginx"
            db = container "DB" "Storage" "PostgreSQL"
        }
        mail = softwareSystem "Mail"

        alice -> bank "Uses" "HTTPS"
        web -> db "Reads and writes" "JDBC"
        bank -> mail "Sends via" "SMTP"
    }
    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
        styles {
            element "Person" {
                background #112233
            }
        }
    }
}
