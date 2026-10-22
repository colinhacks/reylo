select jsonb_build_object('q', coalesce($1::text, 'd'), 'n', coalesce($2::int, 0), 'id', p.id) as j from posts p
