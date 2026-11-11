select * from (select user_id, count(*) as n from posts group by user_id) s where s.n = :n
