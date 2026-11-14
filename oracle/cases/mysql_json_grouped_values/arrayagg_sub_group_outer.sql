select u.id, (select json_object('a', json_arrayagg(p.score), 'm', max(p.n)) from posts p group by u.id) as o from users u
