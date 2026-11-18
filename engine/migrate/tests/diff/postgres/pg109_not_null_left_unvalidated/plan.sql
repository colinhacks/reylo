create table fresh (
  id integer not null,
  d integer,
  constraint fresh_pkey primary key (id)
);
alter table fresh add constraint fresh_d_not_null not null d not valid;
alter table readings alter column a drop not null;
alter table readings add constraint readings_a_not_null not null a not valid;
alter table readings add constraint readings_b_not_null not null b not valid;
alter table readings alter column c drop not null;
