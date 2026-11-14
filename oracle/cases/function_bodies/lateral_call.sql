select p.title, c.body from posts p cross join lateral comments_for(p.id) c
