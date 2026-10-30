with s (id, x) as (select title, id from posts) select * from s where s.id = :t
