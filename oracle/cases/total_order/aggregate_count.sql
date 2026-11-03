select author_id, count(*) as n from posts group by author_id order by n desc, author_id
