alter type mood rename to mood__old;
create type mood as enum ('sad', 'ok');
alter table users alter column mood type mood using mood::text::mood;
drop type mood__old;
