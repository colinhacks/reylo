select x.j from holder2, lateral (select to_jsonb(c) as j from replies c) x
