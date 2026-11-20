create table foobar (foo text, bar text, unique (foo, bar)) partition by list (foo);
create table foobar_1 partition of foobar for values in ('foo_1');
create unique index foobar_unique_idx on foobar (foo, bar);
create table foobar_fk (foo text, bar text, foreign key (foo, bar) references foobar (foo, bar)) partition by list (foo);
create table foobar_fk_1 partition of foobar_fk for values in ('foo_1');
