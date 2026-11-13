select (select (select json_object('k', x.name) from posts p order by p.id limit 1) from users x where false having count(*) >= 0) as o
