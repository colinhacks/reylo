select p.title, t.name, t.age from posts p left join t on t.id = p.author_id where t.name + 1 = 2
