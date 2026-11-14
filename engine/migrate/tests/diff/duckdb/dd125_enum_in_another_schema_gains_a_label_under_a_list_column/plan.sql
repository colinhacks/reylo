drop type s.mood;
create type s.mood as enum('a', 'b', 'c');
alter table s.t alter column ms set data type enum('a', 'b', 'c')[];
alter table s.t alter column m set data type s.mood;
