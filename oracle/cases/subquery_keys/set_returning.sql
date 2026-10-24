select * from (select id, unnest(array[1, 2]) as x from posts) s where s.id = :id
