-- JSON built in a subquery in FROM, a WITH query or a LATERAL subquery, read by the statement
-- around it, and JSON built in one branch of a CASE.
create table member (id int primary key, first_name text not null);
create table team (id int primary key, name text not null, motto text);
create table member_team (member_id int not null references member (id), team_id int not null references team (id));
