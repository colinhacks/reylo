create table t (id integer primary key, aa integer check (aa > 0), n bigint, s varchar not null, e varchar default 'd'); create index te on t (e); comment on index te is 'by e';
