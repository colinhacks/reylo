alter table t alter column c drop default;
alter table t alter column c type text using c::text;
alter type p0 alter attribute a type bigint;
alter table t alter column c type p0 using c::text::p0;
alter table t alter column c set default row((0)::bigint, 'x'::text)::p0;
alter table t alter column c set default row(0, 'x')::p0;
