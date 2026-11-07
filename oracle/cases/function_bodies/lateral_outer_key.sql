select p.id from posts p cross join lateral one_comment(p.author_id) c where c.id = :id::int8
