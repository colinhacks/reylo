select jsonb_build_object('x', coalesce($1::nullable(int), 1.5), 'id', p.id) as j from posts p
