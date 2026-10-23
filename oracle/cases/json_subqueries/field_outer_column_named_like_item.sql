select (select jsonb_build_object('p', p) from posts p where p.id = 1) as j from users
