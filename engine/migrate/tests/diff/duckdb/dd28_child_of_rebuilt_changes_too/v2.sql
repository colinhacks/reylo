create table a (id bigint primary key, n integer check (n >= 0)); create table b (id bigint primary key, a_id bigint references a (id), note varchar, extra integer check (extra > 0));
