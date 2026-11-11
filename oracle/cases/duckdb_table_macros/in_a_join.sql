select p.title, c.body from posts p join comments_for(p.id) c on true
