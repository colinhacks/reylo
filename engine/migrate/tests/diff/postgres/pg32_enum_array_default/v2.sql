create type mood as enum ('sad', 'ok');
create table people (id integer primary key, mood mood not null default 'ok', history mood[] not null default '{ok}');
