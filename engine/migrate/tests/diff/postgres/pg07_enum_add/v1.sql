create type mood as enum ('sad', 'ok');
create table users (id integer primary key, full_name text not null, mood mood not null);
