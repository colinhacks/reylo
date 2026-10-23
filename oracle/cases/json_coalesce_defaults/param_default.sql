select jsonb_build_object('q', coalesce($1::nullable(text), 'd'), 'n', coalesce($2::nullable(int), 0), 'id', p.id) as j from posts p
