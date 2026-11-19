alter type mood rename to mood__old;
create type mood as enum ('a');
alter table t alter column c drop default;
alter table t alter column c type text using c::text;
alter type p alter attribute x type mood;
alter type p alter attribute y type mood;
alter table t alter column c type p using c::text::p;
alter table t alter column c set default row('a'::mood, 'a'::mood)::p;
drop type mood__old;
