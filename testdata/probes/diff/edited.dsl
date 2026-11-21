workspace "Diff probe" "P4-M0 fixture pair — edited side" {
    model {
        mallory = person "Mallory" "Adversary"
        staff = group "Staff" {
            alice = person "Alice" "Primary user, promoted"
        }
        mail = softwareSystem "Mail" "SMTP relay" "Mailgun"
        bank = softwareSystem "Bank" "Core banking" {
            web = container "Web" "Front end" "nginx"
            db = container "DB" "Storage" "PostgreSQL"
        }

        alice -> bank "Uses over HTTPS" "HTTPS"
        web -> db "Reads and writes" "JDBC"
        bank -> mail "Sends via" "SMTP"
    }
    views {
        systemLandscape "landscape" {
            include *
        }
        styles {
            element "Person" {
                background #445566
            }
            element "Software System" {
                background #aabbcc
            }
        }
    }
}
