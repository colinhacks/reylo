select x.id from ci_tab t, lateral (select u.id from u where u.code = t.c) x where t.id = 1
