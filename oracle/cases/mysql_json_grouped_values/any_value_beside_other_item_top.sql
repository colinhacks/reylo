select json_object('k', any_value(u.name)) as o, count(*) as n from users u where false
