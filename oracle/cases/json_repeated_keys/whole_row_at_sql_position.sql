select array_agg(s) as v from (select 1 as a, null::int as a) s
