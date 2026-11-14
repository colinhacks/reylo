select l.j from users u, lateral (select jsonb_build_object('n', count(*), 't', max(p.title), 'a', jsonb_agg(p.id)) as j from posts p where p.user_id = u.id group by p.tag) l
