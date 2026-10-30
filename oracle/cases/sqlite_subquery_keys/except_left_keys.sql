select * from (select id, title from posts except select id, title from posts where id < 0) s where s.id = :id
