create table t (id integer, d varchar, day date default cast(now() as DATE), u varchar, half double generated always as ((cast(id as DOUBLE) / 2)) virtual, primary key (id), check (((u is null) or (cast(u as UUID) is not null))));
create index t_day on t((cast(d as DATE)));
create view "days" as select cast(d as DATE) as "day" from t;
