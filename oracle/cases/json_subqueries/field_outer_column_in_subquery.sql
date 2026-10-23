select jsonb_build_object('x', (select jsonb_build_object('n', (select count(p) from posts p where false) + 0) from posts p2 limit 1)) as j from users u
