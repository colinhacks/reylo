select json_object('t', p.title, 'a', json_arrayagg(p.id)) as o from posts p group by p.title with rollup
