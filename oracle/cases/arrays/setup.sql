-- Postgres arrays hold NULL elements whatever their column's constraints say.
create table teams (id int primary key, tags text[] not null default '{}', scores int[]);
create table members (id int primary key, team_id int references teams (id), email text not null, nickname text);
create table grids (id int primary key, cells int[] not null);
insert into grids values (1, '{1,2}'), (2, '{3,4}');
