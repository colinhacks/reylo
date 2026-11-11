select s.author_id, s.n from (select author_id, count(*) as n from posts group by author_id) s
