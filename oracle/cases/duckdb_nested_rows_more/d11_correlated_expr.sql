select p.id, array(select c from (select p.id + 1 as m, x.big + 1 as b from comments x where x.post_id = p.id) c) as arr from posts p
