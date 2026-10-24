select * from (select title from posts) s where s.title = :t
