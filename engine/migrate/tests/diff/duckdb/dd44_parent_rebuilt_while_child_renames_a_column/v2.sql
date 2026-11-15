create table p (id bigint primary key, n integer check (n > 0)); create table c (id bigint primary key, p_id bigint references p (id), memo varchar);
