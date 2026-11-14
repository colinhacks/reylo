create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, a date default current_date, b timestamp with time zone default now(), c integer default cast((random() * 10) as integer), d uuid default gen_random_uuid(), e struct(x integer, "Y z" varchar) default struct_pack(x := 1, "Y z" := 'q'), f map(varchar, integer) default "map"(list_value('k'), list_value(1)), g integer[3] default list_value(1, 2, 3), h interval default cast('1 day' as interval), i blob default cast('\x41\x00' as blob), j boolean default true, k integer default -1, l double default 1e10, m varchar default 'it''s; -- not a comment', n varchar default 'a
b', o bigint default nextval('s'), p varchar[] default list_value('A', 'b'), q decimal(10,2) default 3.14, r timestamp default '2026-01-02 03:04:05.678', u union(n integer, s varchar) default 'S', primary key (id), check ((k < 0)));
insert into t (id, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, u) select id, a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p, q, r, u from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
