select l.k from posts p cross join lateral (select x.big + 1 as k from comments x right join posts q on q.id = x.post_id where q.id = p.id) l
