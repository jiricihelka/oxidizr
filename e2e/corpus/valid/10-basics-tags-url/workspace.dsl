workspace "Tagged Estate" "Tags and URLs on top-level elements" {

    model {
        analyst = person "Risk Analyst" "Reviews exposure reports" "Internal,Staff" {
            url "https://intranet.example.com/teams/risk"
        }

        risk = softwareSystem "Risk Engine" "Scores portfolios" "Critical,Backend" {
            url "https://risk.example.com"
        }

        analyst -> risk "asks questions of"
    }

    views {
        systemContext risk "context" {
            include *
            autoLayout
        }
    }

}
