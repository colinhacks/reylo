select author_id, json_arrayagg(title returning jsonb) as titles from posts group by author_id order by titles, author_id
