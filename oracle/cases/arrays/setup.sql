-- Postgres arrays hold NULL elements whatever their column's constraints say.
create table teams (id int primary key, tags text[] not null default '{}', scores int[]);
create table members (id int primary key, team_id int references teams (id), email text not null, nickname text);
create table grids (id int primary key, cells int[] not null);
insert into grids values (1, '{1,2}'), (2, '{3,4}');
-- A domain over an array: an array of it holds the domain's values, each one value, where an
-- aggregate of it builds one more dimension of the base type.
create domain int_list as int[];
create table lists (id int primary key, x int_list, y int_list not null default '{1}');
-- Cast or computed to its base type, the domain's array is an array again, which an array of it
-- nests a level deeper.
insert into lists values (1, '{1,NULL}', '{2}');
