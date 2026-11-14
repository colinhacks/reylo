select x.j from cc, lateral (select jsonb_build_object('rows', (select jsonb_agg(r) from replies r), 'c', c) as j from replies c) x
