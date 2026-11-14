create type mood as enum('a', 'b');
create type moods as enum('a', 'b')[];
create table t (id integer, ms moods, primary key (id));
