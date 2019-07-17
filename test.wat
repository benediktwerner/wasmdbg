(module
    (start $start)
    (func $add (param $a i32) (param $b i32) (result i32)
        local.get $a
        local.get $b
        i32.add
    )
    (func (result i32)
        i32.const 17
        i32.const 25
        call $add
    )
    (func $start
        i32.const 0
        if
            f64.const 0xabc
            drop
        else
            i64.const 0x1337
            f64.reinterpret/i64
            drop
        end
    )
)
        
