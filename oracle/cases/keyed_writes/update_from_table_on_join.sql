update users u set name = 'x' from notes n where u.id = n.user_id returning u.id
