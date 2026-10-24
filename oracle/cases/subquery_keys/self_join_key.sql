select * from (select a.id, b.title from posts a join posts b on b.id = a.id) s where s.id = :id
