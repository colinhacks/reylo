select (select json_object('n', count(*), 's', (select json_object('g', group_concat(p.title order by x.id)) from posts p)) from users x where false) as o
