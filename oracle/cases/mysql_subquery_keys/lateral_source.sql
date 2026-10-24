select s.id from users u join lateral (select p.id, u.id as uid from posts p) s where u.id = s.uid and s.id = :id
