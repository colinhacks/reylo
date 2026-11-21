SELECT id, name, email, big, n, at, tags, flag, role, attributes, id + 1 AS next_id
FROM users
WHERE id = {id:UInt32} AND name != {skip:String}
LIMIT 1
