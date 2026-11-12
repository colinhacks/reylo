select (select jsonb_build_object('n', count(*) + u.id, 'm', max(p.n) + u.id, 'nm', u.name || count(*)) from posts p where p.user_id = u.id group by p.tag order by p.tag limit 1) as j from users u
