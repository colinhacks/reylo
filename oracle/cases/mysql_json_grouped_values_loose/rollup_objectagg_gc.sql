select json_object('o', json_objectagg(p.title, p.id), 'g', group_concat(p.title), 't', p.user_id) as o from posts p group by p.user_id with rollup
