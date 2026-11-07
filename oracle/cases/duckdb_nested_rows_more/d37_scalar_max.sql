select array(select c from (select p.id, (select max(x.big) from comments x where x.post_id = p.id) as m from posts p) c) as arr
