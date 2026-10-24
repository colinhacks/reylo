select * from (select a.id from posts a join posts b on b.author_id = a.author_id) s where s.id = :id
