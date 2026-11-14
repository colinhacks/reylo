drop type b.pair;
drop type mood;
create type mood as enum('m', 'n');
create type b.pair as struct(x enum('m', 'n'));
alter table b.t alter column p set data type b.pair;
