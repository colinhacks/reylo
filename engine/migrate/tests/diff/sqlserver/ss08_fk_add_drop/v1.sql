create table teams (id int not null primary key);
create table people (id int not null primary key, team_id int null foreign key references teams (id), boss_id int null);
