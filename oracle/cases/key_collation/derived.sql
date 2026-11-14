select u.id from (select c from ci_tab where id = 1) t, u where u.code = t.c
