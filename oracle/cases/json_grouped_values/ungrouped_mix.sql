select jsonb_build_object('titles', jsonb_agg(p.title), 'n', count(*)) as j from posts p
