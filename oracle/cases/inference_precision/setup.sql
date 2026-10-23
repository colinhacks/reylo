-- The docs' schema, for three rules: a null-rejecting test in the WHERE proves the joined row and
-- the tested column; an aggregate that is NULL only over no rows is never NULL under a plain GROUP
-- BY (unless a FILTER or a window frame can leave it none); and a table's whole row, aggregated or
-- read whole, is never NULL unless an outer join can null it, its string columns keeping their
-- CHECK's union, and a row the outer join may null keeps its own fields' nullability inside the
-- JSON object it makes: a table's NOT NULL, or what a view, a derived table or a WITH query says
-- of its own columns. Users without posts, posts without a date and a row of each kind are here,
-- so a wrong non-null meets a NULL.
create extension if not exists pg_jsonschema;
create table users (id int primary key, name text not null, email text);
create table posts (id int primary key, user_id int references users (id), title text not null, published_at timestamptz);
create table events (id int primary key, kind text not null check (kind in ('click', 'key')), payload jsonb not null check (jsonb_matches_schema('{"type": "object", "properties": {"x": {"type": "integer"}, "y": {"type": "integer"}, "code": {"type": "string"}}}', payload)), note text);
insert into users values (1, 'Ada', 'ada@example.com'), (2, 'Grace', null), (3, 'Linus', null);
insert into posts values (1, 1, 'Hello', '2026-01-01'), (2, 1, 'Draft', null), (3, 2, 'Notes', null);
insert into events values (1, 'click', '{"x": 1, "y": 2}', null), (2, 'key', '{"code": "a"}', 'n'), (3, 'click', '{}', null);
create view post_titles as select id, user_id, title, published_at, upper(title) as shout from posts;
