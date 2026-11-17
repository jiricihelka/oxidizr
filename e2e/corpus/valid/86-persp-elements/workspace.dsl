workspace "Perspectived Estate" "Perspectives shorthand on elements" {

    model {
        analyst = person "Analyst" "Reads reports" {
            perspectives {
                "Security" "Handles personal data"
                "Technical Debt" "Low - new build" "Low"
            }
        }

        reporting = softwareSystem "Reporting" "BI reports" {
            perspectives {
                "Compliance" "Feeds the regulator"
            }
        }

        analyst -> reporting "queries"
    }

    views {
        systemLandscape "landscape" {
            include *
            autoLayout
        }
    }

}
