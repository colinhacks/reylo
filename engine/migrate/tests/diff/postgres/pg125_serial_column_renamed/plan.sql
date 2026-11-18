create sequence counters_new_fizz_seq as integer increment by 1 minvalue 1 maxvalue 2147483647 start with 1 cache 1 no cycle;
-- counters.fizz is renamed new_fizz, keeping its rows. If they are two columns, write instead:
--   alter table counters drop column fizz;
--   alter table counters add column new_fizz integer default nextval('public.counters_new_fizz_seq'::regclass) not null;
alter table counters rename column fizz to new_fizz;
alter table counters rename constraint counters_fizz_not_null to counters_new_fizz_not_null;
alter table counters alter column new_fizz set default nextval('public.counters_new_fizz_seq'::regclass);
drop sequence counters_fizz_seq;
alter sequence counters_new_fizz_seq owned by counters.new_fizz;
