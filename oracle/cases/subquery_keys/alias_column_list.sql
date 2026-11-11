select * from (select id, title from posts) s (pid, t) where s.pid = :id
