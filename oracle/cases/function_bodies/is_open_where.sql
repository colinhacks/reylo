select t.id from posts t where t.author_id = :a::int8 and is_open(t)
