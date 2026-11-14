create table a (id bigint primary key); create table b (id bigint, a_id bigint references a (id)); create table keep (id bigint);
