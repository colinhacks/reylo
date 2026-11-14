create type status as enum ('a', 'c');
create table orgs (id integer primary key, st status default 'a');
create table teams (id integer primary key, org_id integer references orgs (id));
create table members (id integer primary key, team_id integer references teams (id), st status default 'a');
create table assignments (id integer primary key, member_id integer references members (id), st status default 'a');
