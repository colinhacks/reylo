create table foobar (foo text, bar text, unique (foo, bar));
create unique index foobar_unique_idx on foobar (foo, bar);
create table foobar_fk (foo text, bar text, foreign key (foo, bar) references foobar (foo, bar));
