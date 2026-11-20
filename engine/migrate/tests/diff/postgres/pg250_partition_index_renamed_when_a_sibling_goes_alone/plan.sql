drop index i0;
create index i0 on only t (id);
-- index t_p0_id_idx1 is renamed t_p0_id_idx, keeping its rows. If they are two columns, write instead:
--   drop index t_p0_id_idx1;
--   create index t_p0_id_idx on t_p0 using btree (id);
alter index t_p0_id_idx1 rename to t_p0_id_idx;
