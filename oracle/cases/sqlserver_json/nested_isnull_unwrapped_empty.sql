select (select u.id, isnull((select top 1 p.id from posts p where p.user_id = u.id and 1 = 0 for json path, without_array_wrapper), '{}') as first from users u for json path) as doc
