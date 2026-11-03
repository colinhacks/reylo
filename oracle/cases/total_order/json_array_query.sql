select p.id from posts p order by json_array(select q.title from posts q where q.author_id = p.author_id returning jsonb), p.id
