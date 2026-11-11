select * from (select id, title from posts) s (pid) where s.pid = :id
