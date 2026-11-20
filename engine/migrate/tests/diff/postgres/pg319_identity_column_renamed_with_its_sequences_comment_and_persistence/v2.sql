create unlogged table t (id int primary key, new bigint generated always as identity);
comment on sequence t_new_seq is 'ids';
alter sequence t_new_seq set logged;
create table t1 (id int primary key, new bigint generated always as identity);
create table t2 (id int primary key, new bigint generated always as identity);
comment on sequence t2_new_seq is 'the second';
