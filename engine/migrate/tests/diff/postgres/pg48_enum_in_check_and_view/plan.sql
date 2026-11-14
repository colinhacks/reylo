drop view happy_people;
alter table people drop constraint people_mood_check;
alter type mood rename to mood__old;
create type mood as enum ('sad', 'ok');
alter table people alter column mood type mood using mood::text::mood;
alter table people add constraint people_mood_check check (mood <> 'sad');
drop type mood__old;
create view ok_people as select id from people where mood = 'ok';
