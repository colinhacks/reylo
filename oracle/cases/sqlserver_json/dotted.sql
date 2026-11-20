select (select u.id, u.name as [who.name], u.nick as [who.nick] from users u for json path) as people
