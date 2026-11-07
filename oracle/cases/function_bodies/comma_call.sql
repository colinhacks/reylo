select p.title, c.body from posts p, comments_for(p.id) c
