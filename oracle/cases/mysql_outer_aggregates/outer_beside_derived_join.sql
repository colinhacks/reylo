select b.id, (select d.c from users u join (select count(name) as c from comments) d on true limit 1) as x from nobody b
