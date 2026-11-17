select isnull((select top 1 p.id from posts p where p.id = @id for json path, without_array_wrapper), '{}') as o
