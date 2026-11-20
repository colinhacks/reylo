alter table t drop constraint t_pkey;
alter table u drop constraint u_pkey;
alter table t add constraint t_pkey primary key (id) with (fillfactor=65);
alter table u add constraint u_pkey primary key (id) with (fillfactor=70);
