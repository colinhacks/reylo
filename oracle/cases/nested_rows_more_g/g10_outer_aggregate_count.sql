select array(select c from (select count(p.id) as n, max(p.id) as m) c) as arr from posts p where p.id > 100
