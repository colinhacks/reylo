select * from (select id, title from posts) s (title, id) where s.id = :t
