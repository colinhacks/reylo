create type item__old as (a integer, doc json);
alter table t alter column v type item__old using v::text::item__old;
alter type item drop attribute doc;
alter type item drop attribute a;
alter type item add attribute doc json;
alter type item add attribute a integer;
alter type item add attribute b text;
alter table t alter column v type item using case when num_nulls(v) = 1 then null else row((v).doc::json, (v).a::integer, null::text)::item end;
drop type item__old;
