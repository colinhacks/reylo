create macro zz (x) as ((x + 2));
create view a_view as select x.zz() as y from (select 1 as x);
