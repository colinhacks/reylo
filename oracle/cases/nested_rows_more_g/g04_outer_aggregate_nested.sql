select array(select d from (select x.id, array(select c from (select max(p.id) as m) c) as rs from comments x) d) as arr from posts p where p.id > 100
