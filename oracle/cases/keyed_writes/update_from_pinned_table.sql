update users u set name = 'x' from notes n where n.id = :id and u.id = n.user_id returning u.id
