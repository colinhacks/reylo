create sequence s; create table t (id integer primary key,
        a date default current_date, b timestamptz default now(), c integer default (random() * 10)::integer, d uuid default gen_random_uuid(),
        e struct(x integer, "Y z" varchar) default {'x': 1, 'Y z': 'q'}, f map(varchar, integer) default map {'k': 1}, g integer[3] default [1, 2, 3],
        h interval default interval '1 day', i blob default '\x41\x00'::blob, j boolean default true, k integer default -1, l double default 1e10,
        m varchar default 'it''s; -- not a comment', n varchar default e'a\nb', o bigint default nextval('s'), p varchar[] default ['A', 'b'],
        q decimal(10, 2) default 3.14, r timestamp default '2026-01-02 03:04:05.678', u union(n integer, s varchar) default 'S');
