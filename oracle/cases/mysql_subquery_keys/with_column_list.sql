with s (pid, t) as (select id, title from posts) select * from s where s.pid = :id
