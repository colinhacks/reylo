create table users (id bigint primary key, n int not null);
create index users_n_idx on users (n);
