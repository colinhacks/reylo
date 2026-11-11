select u.id, (select json_object('t', p.title, 'c', count(*)) from posts p group by p.title with rollup order by p.title limit 1) as o from users u group by u.id
