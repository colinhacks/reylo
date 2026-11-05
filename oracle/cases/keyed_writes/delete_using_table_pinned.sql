delete from notes n using users u where n.id = :id and u.id = n.user_id returning n.id, u.name
