create temporary table reylo_copy_t as select * from t;
drop table t;
create table t (id integer, s varchar, x integer, tags varchar[], meta struct(A integer, b varchar), primary key (id), check ((s in ('A', 'B'))), check (regexp_matches(s, '^[A-Z]+$')), check ((x between 1 and 10)), check ((len(tags) < 5)), check ((struct_extract(meta, 'A') > 0)), check (case  when ((x > 5)) then ((s = 'B')) else true end), check ((cast(x as varchar) != '')), check ((coalesce(x, 0) >= 0)), check ((not (s is null))), check ((list_contains(tags, 'X') = false)), check (regexp_full_match(s, '[AB]')), check ((id > 0)));
insert into t (id, s, x, tags, meta) select id, s, x, tags, meta from temp.main.reylo_copy_t;
drop table temp.main.reylo_copy_t;
