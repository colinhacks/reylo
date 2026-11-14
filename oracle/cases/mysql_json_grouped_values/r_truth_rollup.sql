select json_object('t', p.title = 'a', 'b', p.title like 'a%', 'c', count(*) > 0, 'l', true) as o from posts p group by p.title with rollup
