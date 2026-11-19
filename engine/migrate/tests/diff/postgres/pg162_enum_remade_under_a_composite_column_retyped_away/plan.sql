alter table t alter column c type text;
alter type mood rename to mood__old;
create type mood as enum ('a', 'c', 'b');
alter type p alter attribute a type mood;
drop type mood__old;
