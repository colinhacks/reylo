alter type p add attribute x integer;
alter table only "T" alter column c set default row(7.5, 7, 7)::p;
alter table "T_p0" alter column c set default row(7.5, (7)::bigint, 7)::p;
