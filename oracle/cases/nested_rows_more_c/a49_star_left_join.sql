select array(select c from (select * from posts p left join comments x on x.post_id = p.id) c) as arr
