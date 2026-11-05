delete from notes n using users u where u.id = n.user_id returning n.id
