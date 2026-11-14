select json_object('k', any_value(u.name), 'a', json_arrayagg(u.id)) as o from users u where false
