create table foobar (id integer not null, foo text not null) partition by list (foo);
create table foobar_1 partition of foobar for values in ('foo_1');
create index partitioned_idx on only foobar (foo);
create index foobar_1_foo on foobar_1 (foo);
