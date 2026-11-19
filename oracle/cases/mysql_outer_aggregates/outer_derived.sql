select b.id, (select d.c from (select count(b.id) as c from posts p) d limit 1) as x from nobody b
