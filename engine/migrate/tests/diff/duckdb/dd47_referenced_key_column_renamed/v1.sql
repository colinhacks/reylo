create table a (id bigint primary key, name varchar); create table b (id bigint, a_id bigint references a (id));
