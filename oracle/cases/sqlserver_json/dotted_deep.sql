select (select u.id, u.nick as [a.b.c] from users u for json path) as doc
