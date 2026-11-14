select p.id, array(select c from (select x.id, r.body as reply from comments x left join replies r on r.comment_id = x.id where x.post_id = p.id) c) as comments from posts p
