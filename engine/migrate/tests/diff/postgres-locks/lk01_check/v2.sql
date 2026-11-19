create table users (id bigint primary key, n int not null constraint n_pos check (n > 0));
