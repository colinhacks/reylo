select * from (select author_id, count(*) as n from posts group by author_id) s where s.n = :n
