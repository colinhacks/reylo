create table t (id integer primary key, a varchar check (a <> '')); comment on table t is 'it''s
multi; line'; comment on column t.a is 'a "quoted" one';
