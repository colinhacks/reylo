select x.j from cc, lateral (select (select to_jsonb(c) from replies c limit 1) as j) x
