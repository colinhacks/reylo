create temporary table reylo_copy_inv as select * from billing.inv;
drop table billing.inv;
create table billing.inv (id bigint, total integer, primary key (id), check ((total >= 0)));
insert into billing.inv (id, total) select id, total from temp.main.reylo_copy_inv;
drop table temp.main.reylo_copy_inv;
create index inv_total on billing.inv(total);
