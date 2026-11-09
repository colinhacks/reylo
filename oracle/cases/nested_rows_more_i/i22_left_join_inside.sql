select x.j from cc, lateral (select to_jsonb(c) as j from (select 1 as k) k left join replies c on true) x
