drop type pair;
create type pair as struct(m varchar);
drop type mood;
alter table t alter column p set data type pair;
