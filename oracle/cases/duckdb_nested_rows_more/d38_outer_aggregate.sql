select array(select c from (select max(p.id) as m) c) as arr from posts p where p.id > 100
