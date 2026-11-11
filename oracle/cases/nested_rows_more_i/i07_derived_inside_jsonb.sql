select x.j from cc, lateral (select d.j from (select to_jsonb(c) as j from replies c) d) x
