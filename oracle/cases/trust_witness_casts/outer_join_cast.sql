select p.title, t.name from posts p left join t on t.id = p.author_id where t.flag::int8 > 0
