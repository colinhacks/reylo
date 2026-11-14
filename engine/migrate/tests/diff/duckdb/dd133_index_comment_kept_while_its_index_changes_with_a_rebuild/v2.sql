create table t (id integer primary key, qty integer check (qty > 0), n integer); create index ix on t (qty, n); comment on index ix is 'by qty';
