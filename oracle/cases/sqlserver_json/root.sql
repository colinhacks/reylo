select (select id from users for json path, root('users')) as doc
