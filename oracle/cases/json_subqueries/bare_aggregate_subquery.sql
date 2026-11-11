select (select jsonb_agg(jsonb_build_object('id', p2.id)) from posts p2) as ps from users u
