select json_object('x', any_value(u.name), 'n', count(*)) as o from users u where false
