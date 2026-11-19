drop view w;
alter view v alter column b set default 'x'::text;
create view w as
select a,
    b,
    (a + 1) as c
   from t;
alter view w alter column a set default nextval('public.s'::regclass);
alter view w alter column b set default 'x'::text;
