select e.x2 from posts p cross join lateral echo(p.author_id) e
