drop index ti;
drop type s.mood;
create type s.mood as enum('a', 'b', 'c');
create index ti on t((cast(a as s.mood)));
