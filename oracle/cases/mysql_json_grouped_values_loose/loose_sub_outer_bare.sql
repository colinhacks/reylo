select (select json_object('nm', u.name, 'x', concat(u.name, ''), 'c', count(*)) from posts p where p.user_id = u.id group by p.tag order by p.tag limit 1) as o from users u
