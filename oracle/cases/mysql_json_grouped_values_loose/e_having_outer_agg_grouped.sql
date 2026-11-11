select (select json_object('k', u.name, 't', p.title) from posts p group by p.title having count(u.id) >= 0 order by p.title limit 1) as o from users u where false
