select x.j from holder2, lateral (select case when x2 > 0 then to_jsonb(c) end as j from replies c, (select 1 as x2) k) x
