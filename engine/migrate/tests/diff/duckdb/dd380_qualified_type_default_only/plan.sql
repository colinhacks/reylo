alter table t alter column a drop default;
drop type s.mood;
create type s.mood as enum('a', 'b', 'c');
alter table t alter column a set default cast(cast('a' as s.mood) as VARCHAR);
