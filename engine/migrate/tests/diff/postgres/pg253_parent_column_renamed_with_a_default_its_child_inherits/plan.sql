-- file 1 of 2
-- p.c is renamed n, keeping its rows. If they are two columns, write instead:
--   alter table p drop column c;
--   alter table p add column n date default '2024-01-07' not null;
alter table p rename column c to n;
alter table only p alter column n set default '2024-01-07';
alter table p add constraint n_not_null check (n is not null) not valid;
alter table h alter column n set default '2024-01-07'::date;
alter table h add constraint n_not_null check (n is not null) not valid;
-- file 2 of 2
alter table p validate constraint n_not_null;
alter table h validate constraint n_not_null;
alter table p alter column n set not null;
alter table p drop constraint n_not_null;
alter table h alter column n set not null;
alter table h drop constraint n_not_null;
