select (select jsonb_build_object('p', upper(p)) from posts p where p.id = 1) as j from users
