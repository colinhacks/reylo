create table t (id bigint primary key, handle varchar check (handle <> '')); comment on column t.handle is 'what they go by';
