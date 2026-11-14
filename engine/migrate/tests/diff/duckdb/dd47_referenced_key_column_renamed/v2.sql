create table a (aid bigint primary key, name varchar); create table b (id bigint, a_id bigint references a (aid));
