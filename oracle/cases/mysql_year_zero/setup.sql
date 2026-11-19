-- MySQL's default sql_mode refuses a zero date and a partial one, but not a date in year 0000,
-- which a DATE and a DATETIME store and date arithmetic computes: `last_day('0000-02-01')` is
-- 0000-02-28. Their range starts at 0000-01-01.
create table early (id int primary key, d date not null, dt datetime not null);
insert into early values (1, '0000-01-01', '0000-12-31 23:59:59'), (2, '0001-01-01', '0001-01-01 00:00:00');
