select p.id, array(select c from (select x.id, x.big + :k::bigint as s from comments x where x.post_id = p.id) c) as arr from posts p
