select title, c.body from posts, lateral comments_for(id) c
