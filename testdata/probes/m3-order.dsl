// M3 WRITE-CATALOG probe (one oracle run, many surfaces):
// - multi-key free-form properties at workspace/model/element/relationship level
//   -> V-11 HashMap bucket order
// - view serialization: name? generatedKey? enterpriseBoundaryVisible false?
// - ElementView x/y at (0,0) -> V-2
// - array ordering: containers named in reverse id order (numeric-id TreeSet?)
// - styles: TreeSet by tag; Boolean false fields
// - deployment: instances, environment on every node element, instance ids,
//   deploymentGroups ordering
workspace "Probe" {
    model {
        p = person "P" "p desc" "Tag A,Tag B" {
            url "https://example.com/p"
            properties {
                "aaa" "1"
                "bbb" "2"
                "ccc" "3"
                "ddd" "4"
                "eee" "5"
                "fff" "6"
                "ggg" "7"
                "hhh" "8"
                "iii" "9"
                "jjj" "10"
                "kkk" "11"
                "lll" "12"
                "mmm" "13"
            }
        }
        s = softwareSystem "S" {
            z = container "Z-first" "" "Container,X" {
                k = component "K1"
            }
            a = container "A-second"
        }
        p -> s "Uses" "HTTP" {
            tags "customrel"
            properties {
                "r1" "rv1"
                "r2" "rv2"
                "r3" "rv3"
            }
        }
        k -> p "Returns data to" "JSON"

        properties {
            "m1" "mv1"
            "m2" "mv2"
        }

        deploymentEnvironment "Live" {
            blue = deploymentGroup "blue group"
            green = deploymentGroup "green group"
            aws = deploymentNode "AWS" "us-east-1" "Region" {
                deploymentGroup "green,blue"
                ssi = softwareSystemInstance s
                child = deploymentNode "EC2" "t3.medium" "VM" {
                    ci = containerInstance z
                }
                infra = infrastructureNode "ALB" "Load balancer" "AWS ALB"
            }
        }
    }

    views {
        systemLandscape "land" "Landscape" {
            include *
        }
        container s "cont" "Containers" {
            include *
            autoLayout
        }
        filtered "land" "include" "Element,Person" "filt" "Filtered desc" {
            // inherits base view layout
        }
        dynamic z "dyn" "Dynamic desc" {
            k -> p "Returns data to"
        }
        deployment * "Live" "dep" "Deployment desc" {
            include *
        }

        styles {
            element "Tag B" {
                color #ff0000
            }
            element "Element" {
                background #eeeeee
                stroke #000000
                strokeWidth 2
                shape Ellipse
                border Dashed
                opacity 90
                fontSize 22
                metadata false
                description false
                width 100
                height 80
            }
            relationship "Relationship" {
                color #123456
                thickness 3
                dashed true
                routing Orthogonal
                jump true
                style Dotted
                fontSize 11
            }
        }
    }

    configuration {
        scope SoftwareSystem
        properties {
            "cfg" "1"
            "cfg2" "2"
        }
    }

    properties {
        "aaa" "1"
        "bbb" "2"
        "ccc" "3"
        "ddd" "4"
        "eee" "5"
        "fff" "6"
        "ggg" "7"
        "hhh" "8"
        "iii" "9"
        "jjj" "10"
        "kkk" "11"
        "lll" "12"
        "mmm" "13"
    }
}
