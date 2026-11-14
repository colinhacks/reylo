select x.j from cc, lateral (select to_jsonb(j) as j from (replies c cross join (select 1 as k) k) j) x
