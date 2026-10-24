delete b from bans b join users u on u.id = b.member_id where u.id = :id
