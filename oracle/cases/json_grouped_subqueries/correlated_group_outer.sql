select (select jsonb_build_object('m', max(p.n), 'c', count(*), 'a', jsonb_agg(p.score)) from posts p group by u.id) as j from users u
