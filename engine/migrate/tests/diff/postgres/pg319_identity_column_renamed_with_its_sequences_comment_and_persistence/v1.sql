create unlogged table t (id int primary key, old bigint generated always as identity);
comment on sequence t_old_seq is 'ids';
alter sequence t_old_seq set logged;
create table t1 (id int primary key, old bigint generated always as identity);
create table t2 (id int primary key, old bigint generated always as identity);
alter sequence t2_old_seq rename to t1_new_seq;
comment on sequence t1_new_seq is 'the second';
