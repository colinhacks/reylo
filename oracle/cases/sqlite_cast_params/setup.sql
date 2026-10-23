-- A parameter under the cast a `:name::type` compiles to on SQLite, `CAST(? AS type)` or `json(?)`
-- for `::json`, is the cast's type and takes no NULL written whole to a nullable column, where the
-- bare one takes it; neither takes NULL coalesced or tested for NULL, and either takes it cast to
-- `nullable(…)`.
create table users (id integer primary key, name text not null, email text unique, score real, n integer, doc text, meta text not null default '{}') strict;
insert into users (name, email, score, n, doc) values ('Ada', 'ada@example.com', 1.5, 1, '{"a":1}');
