drop type moods;
drop type mood;
create type mood as enum('a', 'b', 'c');
create type moods as enum('a', 'b', 'c')[];
alter table t alter column ms set data type moods;
