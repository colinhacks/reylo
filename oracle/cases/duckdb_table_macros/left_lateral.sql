select p.title, c.body from posts p left join lateral comments_for(p.id) c on true
