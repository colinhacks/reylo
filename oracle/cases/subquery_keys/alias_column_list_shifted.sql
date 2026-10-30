select * from (select title, id from posts) s (id, t) where s.id = :t
