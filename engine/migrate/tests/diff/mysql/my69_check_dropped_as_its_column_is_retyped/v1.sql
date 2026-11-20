create table t (x integer, flag varchar(1), constraint ck_x check (x > 1), constraint my_enum check (flag in ('a', 'b')));
