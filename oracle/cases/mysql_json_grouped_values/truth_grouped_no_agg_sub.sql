select (select json_object('t', u.id = 1, 'f', u.flag) from users u group by u.id order by u.id limit 1) as o
