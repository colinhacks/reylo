drop index t_m;
drop type mood;
create type mood as enum('a', 'b', 'c');
create index t_m on t((cast(m as mood)));
