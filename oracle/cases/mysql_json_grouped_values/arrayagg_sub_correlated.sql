select u.id, (select json_object('a', json_arrayagg(p.score), 'c', count(*) + u.id, 'm', max(p.n) + u.n) from posts p where p.user_id = u.id group by p.tag order by p.tag limit 1) as o from users u
