select * from (select id, row_number() over (order by id) as rn from posts) s where s.id = :id
