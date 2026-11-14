select count(*) as n, (select json_object('k', u.name, 'a', json_arrayagg(p.id)) from posts p group by p.user_id order by p.user_id limit 1) as o from users u where false
