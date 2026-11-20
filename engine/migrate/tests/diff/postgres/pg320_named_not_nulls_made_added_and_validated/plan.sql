-- file 1 of 2
create table w (
  a integer constraint a_required not null
);
create table x (
  b integer constraint x_b_named not null no inherit,
  c integer generated always as identity constraint x_c_named not null
);
alter table u add column b integer default 0 constraint b_required not null;
alter table v add constraint a_not_null check (a is not null) not valid;
alter table v rename constraint v_a_old to v_a_new;
alter table y alter column id set not null;
alter table y alter column id add generated always as identity;
select setval(pg_get_serial_sequence('public.y', 'id'), coalesce(max(id), 0) + 1, false) from y;
alter table y rename constraint y_id_not_null to y_id_named;
alter table z add not null a no inherit;
alter table z rename constraint z_a_not_null to z_a_named;
-- file 2 of 2
alter table v validate constraint a_not_null;
alter table v alter column a set not null;
alter table v drop constraint a_not_null;
