create schema app;
create table app.t_gen (v int default 0, id int generated always as identity);
alter default privileges in schema app grant usage on sequences to diff_seq_reader;
