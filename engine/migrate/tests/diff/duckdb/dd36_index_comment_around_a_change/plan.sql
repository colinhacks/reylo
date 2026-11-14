drop index t_email;
alter table t alter column age set data type bigint;
create index t_email on t(email);
comment on index t_email is 'lookup';
