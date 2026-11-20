-- loses data: foobar_1
-- loses data: foobar_fk_1
-- loses data: foobar_fk
-- loses data: foobar
alter table foobar detach partition foobar_1;
drop table foobar_1;
drop table foobar_fk_1;
drop table foobar_fk;
drop table foobar;
create table foobar (
  foo text,
  bar text,
  constraint foobar_foo_bar_key unique (foo, bar)
);
create table foobar_fk (
  foo text,
  bar text
);
alter table foobar_fk add constraint foobar_fk_foo_bar_fkey foreign key (foo, bar) references foobar(foo, bar);
create unique index foobar_unique_idx on foobar (foo, bar);
