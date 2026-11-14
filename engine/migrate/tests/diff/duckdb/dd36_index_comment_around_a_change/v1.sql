create table t (id bigint primary key, email varchar, age integer); create index t_email on t (email); comment on index t_email is 'lookup';
