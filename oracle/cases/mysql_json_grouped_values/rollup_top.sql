select json_object('t', p.title, 'c', count(*), 'a', json_arrayagg(p.id), 'm', max(p.n)) as o from posts p group by p.title with rollup
