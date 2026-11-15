create schema s; create type s.mood as enum ('a', 'b', 'c'); create table t (a varchar); create index ti on t ((a::s.mood));
