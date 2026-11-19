select b.id, exists (select d.c from users u, (select count(name) as c from comments) d) as e from nobody b
