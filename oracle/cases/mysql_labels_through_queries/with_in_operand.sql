with x as (select a, b from e4) (with x as (select a, b from e3) select a as v from x) union all select b from x
