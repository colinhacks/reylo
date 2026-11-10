select p.id, array(select c from (select sum(x.big) as s, max(x.big) as m, min(x.id) as mi, avg(x.big) as av, sum(x.big) + 1 as s1 from comments x where x.post_id = p.id) c) as arr from posts p
