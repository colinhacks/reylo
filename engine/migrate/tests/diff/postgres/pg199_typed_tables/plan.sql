alter table t of ty;
alter type ty2 add attribute y text cascade;
create table n of ty2;
alter table u alter column a set default 1;
alter table u alter column a set not null;
alter table w alter column y set default 'y'::text;
alter table u add constraint u_pkey primary key (a);
