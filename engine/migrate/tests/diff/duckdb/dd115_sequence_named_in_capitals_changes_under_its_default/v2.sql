create sequence "Seq" increment by 10; create table t (id bigint primary key default nextval('"Seq"'));
