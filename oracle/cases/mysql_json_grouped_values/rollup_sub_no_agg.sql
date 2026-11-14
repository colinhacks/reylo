select (select json_object('t', p.title) from posts p group by p.title with rollup order by p.title limit 1) as o
