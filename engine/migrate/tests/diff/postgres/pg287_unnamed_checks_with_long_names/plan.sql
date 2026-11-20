create table "表格" (
  id integer,
  constraint "表格_id_check" check (id > 0)
);
create table tttttttttttttttttttttttttttttt (
  aaaaaaaaaaaaaaaaaaaaaaaaaaaa_x integer,
  aaaaaaaaaaaaaaaaaaaaaaaaaaaa_y integer,
  constraint tttttttttttttttttttttttttttt_aaaaaaaaaaaaaaaaaaaaaaaaaaa_check1 check (aaaaaaaaaaaaaaaaaaaaaaaaaaaa_y > 0),
  constraint tttttttttttttttttttttttttttt_aaaaaaaaaaaaaaaaaaaaaaaaaaaa_check check (aaaaaaaaaaaaaaaaaaaaaaaaaaaa_x > 0)
);
alter table other add constraint other_id_key unique (id) deferrable;
