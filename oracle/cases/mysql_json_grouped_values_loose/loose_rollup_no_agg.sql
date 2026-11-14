select json_object('u', p.user_id, 't', p.title) as o from posts p group by p.user_id with rollup
