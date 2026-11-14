select p.id, array(select c from (select x.id, r.body as rb, r.id as rid from comments x left join replies r on r.comment_id = x.id where x.post_id = p.id) c) as arr from posts p
