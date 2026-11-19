create sequence bindings_id_seq as bigint increment by 1 minvalue 1 maxvalue 9223372036854775807 start with 1 cache 1 no cycle;
create table bindings (
  id bigint default nextval('public.bindings_id_seq'::regclass) not null,
  kind text not null,
  target bigint not null
);
create table bindings_a (
  constraint kind_is_a check (kind = 'a'),
  constraint bindings_a_target_kind_key unique (target, kind)
) inherits (bindings);
create table bindings_b (
  kind text default 'b' not null,
  note text
) inherits (bindings);
alter sequence bindings_id_seq owned by bindings.id;
