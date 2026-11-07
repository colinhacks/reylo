-- A key an object is built with twice holds its last value: `jsonb` keeps that pair alone, and
-- `json` keeps both pairs in its text, of which a parser keeps the last.
create table t (id bigint primary key, name text not null);
insert into t values (1, 'a');
