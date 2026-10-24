select * from (select cast(id as text) as id, title from posts) s where s.id = :id
