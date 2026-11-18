alter table down alter column id set increment by 1 set minvalue 1 set maxvalue 2147483647 set start with 1 restart;
alter table turned alter column id set increment by -1;
alter table up alter column id set increment by -1 set minvalue -2147483648 set maxvalue -1 set start with -1 restart;
