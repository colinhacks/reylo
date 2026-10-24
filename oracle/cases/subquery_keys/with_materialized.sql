with s as materialized (select id, title from posts) select * from s where s.id = :id
