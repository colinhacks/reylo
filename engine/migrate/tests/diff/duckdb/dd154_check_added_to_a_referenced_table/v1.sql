create table authors (id bigint primary key, name varchar); create table books (id bigint primary key, author_id bigint references authors (id), pages integer);
