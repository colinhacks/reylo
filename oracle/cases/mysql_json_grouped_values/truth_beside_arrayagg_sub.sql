select (select json_object('t', u.id = 1, 'a', json_arrayagg(p.id = 1)) from users u join posts p on p.user_id = u.id group by u.id order by u.id limit 1) as o
