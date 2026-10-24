select * from (select author_id, title, count(*) as n from posts group by author_id, title) s where s.author_id = :a
