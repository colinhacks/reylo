create temporary table reylo_copy_ev as select * from ev;
drop table ev;
create table ev (id integer, amount decimal(12,2), primary key (id));
insert into ev (id, amount) select id, amount from temp.main.reylo_copy_ev;
drop table temp.main.reylo_copy_ev;
create index ev_amount on ev(amount);
