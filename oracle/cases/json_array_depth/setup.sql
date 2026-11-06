-- A Postgres array of any number of dimensions inside a document, whose type says `any_depth`:
-- `int[]` holds `{1,2}` and `{{1,2}}` alike, and both are JSON arrays there, the second of arrays.
create table t (id bigint primary key, a int[] not null, s text[]);
insert into t values (1, '{{1,2},{3,4}}', '{{{x}}}'), (2, '{1,2}', '{y}');
