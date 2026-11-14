-- loses data: c
drop table c;
-- DuckDB makes s again, so it starts again at 1: where rows hold its values, start it past them
alter table p alter column k drop default;
drop sequence s;
create sequence s start with 1 increment by 2 minvalue 1 maxvalue 9223372036854775807;
alter table p alter column k set default nextval('s');
create table c2 (id integer, p_id bigint, q bigint, primary key (id), foreign key (p_id) references p (id), check ((q < 100)));
