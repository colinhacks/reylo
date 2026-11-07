select x.j from cc, lateral (with w as (select 1 as k) select to_jsonb(c) as j from replies c, w) x
