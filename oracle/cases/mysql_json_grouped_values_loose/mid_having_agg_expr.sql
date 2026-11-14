select (select (select json_object('k', concat(x.name, ''), 'c', count(*)) from posts p group by p.user_id order by p.user_id limit 1) from users x where false having count(*) >= 0) as o
