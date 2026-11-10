select array(select c from (select max(p.id) as m, min(p.title) as t, sum(p.id) as s) c) as arr from posts p where p.id > 100
