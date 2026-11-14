create type mood as enum ('sad', 'ok', 'happy');
create table people (id integer primary key, mood mood not null check (mood <> 'sad'));
create view happy_people as select id from people where mood = 'happy';
