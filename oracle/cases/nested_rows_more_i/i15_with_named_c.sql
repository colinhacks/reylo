select x.j from cc, lateral (with c as (select 1 as id) select to_jsonb(c) as j from c) x
