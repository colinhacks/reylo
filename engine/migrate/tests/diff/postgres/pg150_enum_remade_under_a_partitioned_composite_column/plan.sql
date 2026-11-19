alter type mood rename to mood__old;
create type mood as enum ('a', 'b');
alter table t alter column c type text using c::text;
alter type p0 alter attribute a type mood;
alter table t alter column c type p0 using c::text::p0;
drop type mood__old;
