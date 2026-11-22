# oxidizr-layout

The pure graph layout engine: a neutral `LayoutGraph` (measured nodes,
edges with optional anchors) in, node placements and edge polylines out.

`LayoutEngine` is the seam; `Layered` is the shipped engine (rank
assignment, ordering, coordinate pass with channel lanes). An optional
graphviz `dot` backend sits behind the `dot-backend` cargo feature
(off by default).

No GUI types, no IO — deterministic, benchmarked at 1k-node scale.

Consumed by `oxidizr-viewer` (display-time and explicit auto-layout).
