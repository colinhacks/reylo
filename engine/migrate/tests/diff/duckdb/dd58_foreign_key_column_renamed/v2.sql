create table a (id bigint primary key); create table b (id bigint, author_id bigint references a (id));
