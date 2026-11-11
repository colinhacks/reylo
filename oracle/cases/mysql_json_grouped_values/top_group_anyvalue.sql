select json_object('x', any_value(p.title), 'n', count(*)) as o from posts p group by p.user_id
