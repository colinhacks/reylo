alter type e rename to e__old;
create type e as enum ('a', 'c');
alter table p alter column c type e using c::text::e;
alter table x alter column c type e using c::text::e;
drop type e__old;
alter table x rename constraint x_id_not_null to p_id_not_null;
alter table p attach partition x for values from (1) to (4);
