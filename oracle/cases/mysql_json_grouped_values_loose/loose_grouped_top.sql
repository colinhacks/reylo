select json_object('t', p.title, 'x', concat(p.title, ''), 's', p.score, 'n', count(*), 'a', json_arrayagg(p.id)) as o from posts p group by p.user_id
