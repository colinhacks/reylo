select * from (select author_id, count(*) as n from posts group by 1) s where s.author_id = :a
