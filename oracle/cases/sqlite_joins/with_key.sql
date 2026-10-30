with s as (select id, name from users) select * from s where s.id = :id
