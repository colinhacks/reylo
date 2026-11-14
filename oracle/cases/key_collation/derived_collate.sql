select u.id from (select 'a' collate ci as c) t, u where u.code = t.c
