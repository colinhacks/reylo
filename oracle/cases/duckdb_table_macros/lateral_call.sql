select p.title, c.body from posts p, lateral comments_for(p.id) c
