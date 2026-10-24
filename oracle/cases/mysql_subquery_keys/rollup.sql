select * from (select author_id, count(*) as n from posts group by author_id with rollup) s where s.author_id = :a
