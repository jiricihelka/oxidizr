workspace extends base.dsl {

    model {
        !include spokes.dsl

        shop -> hub "orders via"
        hub -> wms "dispatches to"
    }


