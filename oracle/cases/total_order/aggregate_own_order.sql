select author_id, string_agg(title, ',' order by score) as titles from posts group by author_id order by titles, author_id
