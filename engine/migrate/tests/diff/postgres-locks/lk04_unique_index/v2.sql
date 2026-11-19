create table users (id bigint primary key, n int not null);
create unique index users_n_key on users (n);
