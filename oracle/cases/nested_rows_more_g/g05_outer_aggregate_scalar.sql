select (select c from (select max(p.id) as m) c) as one from posts p where p.id > 100
