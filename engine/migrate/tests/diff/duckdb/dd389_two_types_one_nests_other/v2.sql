create type mood as enum ('a', 'b', 'c'); create type pair as struct(m mood, n integer); create table t (p pair, check ((p).n > 0 or p is null));
