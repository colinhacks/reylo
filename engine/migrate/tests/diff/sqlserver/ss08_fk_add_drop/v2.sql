create table teams (id int not null primary key);
create table people (id int not null primary key, team_id int null, boss_id int null, constraint people_boss foreign key (boss_id) references people (id));
