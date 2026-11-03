create type mood as enum ('sad', 'ok');
create table people (id integer primary key, mood mood not null check (mood <> 'sad'));
create view ok_people as select id from people where mood = 'ok';
