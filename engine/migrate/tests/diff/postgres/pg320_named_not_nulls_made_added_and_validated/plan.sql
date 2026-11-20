-- file 1 of 2
-- loses data: typed.id
alter table typed drop column id;
alter table typed add column a integer constraint a_typed not null;
alter table typed of ty;
create table w (
  a integer constraint a_required not null
);
create table x (
  b integer constraint x_b_named not null no inherit,
  c integer generated always as identity constraint x_c_named not null
);
alter table h alter column a drop not null;
alter table h add constraint h_a_named not null a no inherit;
alter table k add constraint k_a_named not null a no inherit;
alter table u add column b integer default 0 constraint b_required not null;
alter table v add constraint a_not_null check (a is not null) not valid;
alter table v rename constraint v_a_old to v_a_new;
alter table y add constraint y_id_named not null id;
alter table y alter column id add generated always as identity;
select setval(pg_get_serial_sequence('public.y', 'id'), coalesce(max(id), 0) + 1, false) from y;
alter table z add constraint z_a_named not null a no inherit;
-- file 2 of 2
alter table v validate constraint a_not_null;
alter table v alter column a set not null;
alter table v drop constraint a_not_null;
