create table kept (id integer primary key);
create table bindings (id bigserial, kind text not null, target bigint not null);
create table bindings_a (constraint kind_is_a check (kind = 'a'), unique (target, kind)) inherits (bindings);
create table bindings_b (note text, kind text not null default 'b') inherits (bindings);
