with s as (select id, title from posts) select * from s where s.id = :id
