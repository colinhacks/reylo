alter table app.t_gen alter column id drop expression;
alter table app.t_gen alter column id set not null;
alter table app.t_gen alter column id add generated always as identity;
select setval(pg_get_serial_sequence('app.t_gen', 'id'), coalesce(max(id), 0) + 1, false) from app.t_gen;
revoke usage on sequence app.t_gen_id_seq from diff_seq_reader;
