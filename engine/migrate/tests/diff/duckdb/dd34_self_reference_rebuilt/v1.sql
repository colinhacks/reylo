create table node (id bigint primary key, parent bigint references node (id), name varchar);
