select json_object('n', count(*), 's', (select json_object('g', group_concat(p.title order by u.id), 'c', count(*)) from posts p)) as o from users u where false
