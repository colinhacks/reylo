with comments as (select 1::bigint as id, null::varchar as body union all select 1, null) select id, body from outer_c2(1)
