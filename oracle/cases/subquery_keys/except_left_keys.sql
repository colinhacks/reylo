select * from (select id, title from posts except all select id, title from archive.posts) s where s.id = :id
