delete m from members m join bans b on b.member_id = m.id where m.id = :id
