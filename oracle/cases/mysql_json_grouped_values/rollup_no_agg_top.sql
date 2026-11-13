select json_object('t', p.title, 'u', concat(p.title, '')) as o from posts p group by p.title with rollup
