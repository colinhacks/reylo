select u.id from (select d from ci_tab where id = 1) t, u where u.code = t.d
