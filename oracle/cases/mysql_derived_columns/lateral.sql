select u.id, d.n from users u, lateral (select count(*) as n, max(p.id) as last from posts p where p.user_id = u.id) d
