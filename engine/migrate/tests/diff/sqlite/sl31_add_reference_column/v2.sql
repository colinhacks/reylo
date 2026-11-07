create table teams (id integer primary key);
create table players (id integer primary key, name text not null, team_id integer references teams (id));
