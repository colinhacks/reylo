select p.title, t.plain from posts p left join t on t.id = p.author_id where t.n < 'x'
