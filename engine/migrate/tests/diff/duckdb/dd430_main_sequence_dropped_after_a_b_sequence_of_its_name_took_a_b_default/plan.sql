alter table b.t alter column k drop default;
drop sequence s;
alter table b.t alter column k set default nextval('s');
