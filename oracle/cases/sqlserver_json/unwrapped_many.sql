select (select id from users for json path, without_array_wrapper) as doc
