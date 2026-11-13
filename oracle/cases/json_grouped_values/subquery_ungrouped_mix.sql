select (select jsonb_build_object('titles', jsonb_agg(p.title), 'n', count(*)) from posts p) as j
