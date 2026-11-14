select array(select c from (select max(p.id) + 1 as m, upper(min(p.title)) as t) c) as arr from posts p where p.id > 100
