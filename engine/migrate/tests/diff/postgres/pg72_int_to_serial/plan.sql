create sequence counters_n_seq as integer increment by 1 minvalue 1 maxvalue 2147483647 start with 1 cache 1 no cycle;
alter table counters alter column n set default nextval('public.counters_n_seq'::regclass);
alter sequence counters_n_seq owned by counters.n;
