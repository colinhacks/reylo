update u set code = u.code from ci_tab t where t.id = 1 and u.code = t.c returning u.id
