create type mood as enum ('sad', 'ok', 'happy');
create table users (id integer primary key, full_name text not null, mood mood not null);
