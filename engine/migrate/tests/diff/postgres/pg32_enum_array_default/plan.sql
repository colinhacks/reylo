alter type mood rename to mood__old;
create type mood as enum ('sad', 'ok');
alter table people alter column history drop default;
alter table people alter column history type mood[] using history::text[]::mood[];
alter table people alter column history set default '{ok}'::mood[];
alter table people alter column mood drop default;
alter table people alter column mood type mood using mood::text::mood;
alter table people alter column mood set default 'ok'::mood;
drop type mood__old;
