select p.id, array(select c from (select x.post_id + 1 as k, x.post_id as raw from comments x where x.post_id = p.id group by rollup (x.post_id)) c) as arr from posts p
