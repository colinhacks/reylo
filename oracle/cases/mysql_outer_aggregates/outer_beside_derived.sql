select b.id, (select d.c from users u, (select count(name) as c from comments) d limit 1) as x from nobody b
