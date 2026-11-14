create type mood as enum ('a', 'b'); create type moods as mood[]; create table t (id integer primary key, ms moods);
