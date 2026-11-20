select u.id, json_object('id': u.id, 'nick': u.nick, 'active': u.active) as o from users u
