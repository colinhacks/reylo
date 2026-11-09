select p.id, array(select f from comments_for(p.id) f) as comments from posts p
