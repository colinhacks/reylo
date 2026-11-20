-- loses data: item.gone
alter type item drop attribute gone;
drop type e;
create type item__old as (a integer, b integer);
alter table t alter column v type item__old using v::text::item__old;
alter type item drop attribute b;
alter type item drop attribute a;
alter type item add attribute b integer;
alter type item add attribute a integer;
alter table t alter column v type item using case when num_nulls(v) = 1 then null else row((v).b::integer, (v).a::integer)::item end;
drop type item__old;
