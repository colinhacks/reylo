drop index t_m;
drop type mood;
create type mood as enum('a', 'b', 'c');
create index t_m on t((as_mood(s)));
