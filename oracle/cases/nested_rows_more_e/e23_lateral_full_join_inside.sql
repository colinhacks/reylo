select l.k from posts p cross join lateral (select x.big + p.id as k from comments x full join replies r on false where coalesce(x.post_id, p.id) = p.id) l
