-- JSON a LATERAL subquery builds, read by the statement around it, and JSON one branch of a CASE
-- builds. A WITH query's and any other subquery's is a statement of its own (json_item_shapes).
create table member (id int primary key, first_name text not null);
create table team (id int primary key, name text not null, motto text);
create table member_team (member_id int not null references member (id), team_id int not null references team (id));
-- A table whose motto is never NULL, where team's may be: a probe that read the wrong `m` would show it.
create table guest (id int primary key, motto text not null);
