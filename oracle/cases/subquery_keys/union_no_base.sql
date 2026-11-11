select * from (select id from posts union select 1::bigint) s where s.id = :id
