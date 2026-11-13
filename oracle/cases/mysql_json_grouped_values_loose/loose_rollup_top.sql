select json_object('t', p.title, 'x', concat(p.title, ''), 'c', count(*)) as o from posts p group by p.user_id with rollup
