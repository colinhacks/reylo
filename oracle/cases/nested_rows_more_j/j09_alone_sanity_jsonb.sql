select x.j from lateral (select to_jsonb(c) as j from replies c) x
