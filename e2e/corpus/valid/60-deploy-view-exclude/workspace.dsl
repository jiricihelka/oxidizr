workspace "Deployment Excludes" "Deployment view with excludes" {

    !identifiers hierarchical

    model {
        invoicing = softwareSystem "Invoicing" "Issues invoices" {
            svc = container "Invoice Service" "Core" "Java"
            report = container "PDF Service" "Renders PDFs" "Node.js"
        }

        deploymentEnvironment "Prod" {
            deploymentNode "app-1" {
                svcI = containerInstance invoicing.svc
            }
            deploymentNode "pdf-1" {
                repI = containerInstance invoicing.report
            }
        }
    }

    views {
        deployment invoicing "Prod" {
            include *
            exclude invoicing.report

            autoLayout
        }
    }

}
