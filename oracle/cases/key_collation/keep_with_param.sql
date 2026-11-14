with t as (select :x::text as c) select u.id from t, u where u.code = t.c
