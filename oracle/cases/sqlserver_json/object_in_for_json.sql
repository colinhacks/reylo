select (select u.id, json_object('n': u.name) as o from users u for json path) as doc
