render = softwareSystem "Render Farm" "Charts as a service" {
    gateway = container "Gateway" "API edge" "Go"
    worker = container "Worker" "Headless browser" "Go"
    queue = container "Queue" "Jobs" "Redis"

    gateway -> queue "enqueues"
    worker -> queue "dequeues"
}
