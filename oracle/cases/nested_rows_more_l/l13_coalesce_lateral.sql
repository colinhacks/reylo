select x.j from cc, lateral (select coalesce(to_jsonb(c), '{}') as j from replies c) x
