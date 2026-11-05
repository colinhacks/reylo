update users u set name = 'x' from notes n where u.id = :id and n.user_id = u.id returning u.id, n.body
