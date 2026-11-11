with s (title, id) as (select id, title from posts) select * from s where s.id = :t
