drop index t_email;
alter table t rename column nick to handle;
alter table t alter column age set data type bigint;
create index t_email on t(email);
-- t.nick is renamed handle, keeping its rows. If they are two columns, drop the one and add the other instead.
