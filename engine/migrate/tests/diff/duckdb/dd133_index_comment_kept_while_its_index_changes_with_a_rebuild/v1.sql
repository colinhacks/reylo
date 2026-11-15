create table t (id integer primary key, qty integer); create index ix on t (qty); comment on index ix is 'by qty';
