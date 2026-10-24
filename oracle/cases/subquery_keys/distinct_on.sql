select * from (select distinct on (author_id) author_id, id from posts order by author_id, id) s where s.author_id = :a
