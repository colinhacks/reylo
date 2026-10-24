select * from (select id, title from posts order by id limit 10 offset 5) s where s.id = :id
