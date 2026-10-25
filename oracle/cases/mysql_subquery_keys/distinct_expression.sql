select * from (select distinct author_id, concat(title, '!') as t from posts) s where s.author_id = :a
