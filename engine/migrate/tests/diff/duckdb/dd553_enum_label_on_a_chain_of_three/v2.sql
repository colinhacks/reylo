create type status as enum ('a', 'b', 'c');
create table orgs (id integer primary key, st status default 'a');
create table members (id integer primary key, org_id integer references orgs (id), st status default 'a');
create table assignments (id integer primary key, member_id integer references members (id), st status default 'a');
