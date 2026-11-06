with t as (select c from ci_tab where id = 1) select u.id from t, u where u.code = t.c
