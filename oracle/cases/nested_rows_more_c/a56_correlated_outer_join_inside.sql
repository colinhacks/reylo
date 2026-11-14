select p.id, array(select c from (select p.id + x.big as s from posts q left join comments x on x.post_id = q.id and false where q.id = p.id) c) as arr from posts p
