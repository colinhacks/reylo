select x.j from cc, lateral (select to_jsonb(c) as j from replies c where c.id > cc.id - 100) x
